use anyhow::{Result, anyhow};
use colored::*;
use dirs;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use self_update::self_replace;
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

const GITLAB_PROJECT_PATH: &str = "Zillowe/Zillwen/Zusty/Zoi";
const GITLAB_PROJECT_ID: &str = "71087662";

#[derive(Debug, Deserialize)]
struct GitLabRelease {
    tag_name: String,
}

fn get_latest_tag(branch_prefix: &str) -> Result<String> {
    println!("Fetching latest release information from GitLab...");
    let api_url = format!(
        "https://gitlab.com/api/v4/projects/{}/releases",
        GITLAB_PROJECT_ID
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("Zoi-Upgrader")
        .build()?;
    let releases: Vec<GitLabRelease> = client.get(&api_url).send()?.json()?;

    let latest_tag = releases
        .into_iter()
        .find(|r| r.tag_name.starts_with(branch_prefix))
        .map(|r| r.tag_name)
        .ok_or_else(|| anyhow!("No release found with prefix '{}'", branch_prefix))?;

    println!(
        "Found latest tag for branch prefix '{}': {}",
        branch_prefix,
        latest_tag.green()
    );
    Ok(latest_tag)
}

fn download_file(url: &str, path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download file: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>- "));

    let mut dest = File::create(path)?;
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(&buffer[..bytes_read])?;
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Download complete.");
    Ok(())
}

fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<()> {
    println!("Extracting binary...");
    let file = File::open(archive_path)?;

    if archive_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        let mut archive = ZipArchive::new(file)?;
        archive.extract(target_dir)?;
    } else {
        let tar = ZstdDecoder::new(file)?;
        let mut archive = Archive::new(tar);
        archive.unpack(target_dir)?;
    }
    Ok(())
}

fn verify_checksum(file_path: &Path, checksums_content: &str, filename: &str) -> Result<()> {
    println!("Verifying checksum for {}...", filename);
    let expected_hash = checksums_content
        .lines()
        .find(|line| line.contains(filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or(anyhow!("Checksum not found for {}.", filename))?;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    io::copy(&mut file, &mut hasher)?;
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(anyhow!(
            "Checksum mismatch for {}! The file may be corrupt.",
            filename
        ));
    }
    println!("Checksum verified successfully for {}.", filename.green());
    Ok(())
}

fn get_platform_info() -> Result<(&'static str, &'static str)> {
    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        // "freebsd" => "freebsd",
        // "openbsd" => "openbsd",
        _ => return Err(anyhow!("Unsupported OS: {}", env::consts::OS)),
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(anyhow!("Unsupported architecture: {}", env::consts::ARCH)),
    };
    Ok((os, arch))
}

fn fallback_full_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str,
) -> Result<PathBuf> {
    println!(
        "
Falling back to full binary download..."
    );
    let archive_ext = if os == "windows" { "zip" } else { "tar.zst" };
    let archive_filename = format!("zoi-{os}-{arch}.{archive_ext}");
    let download_url = format!("{base_url}/{archive_filename}");
    let temp_dir = Builder::new().prefix("zoi-full-upgrade").tempdir()?;
    let temp_archive_path = temp_dir.path().join(&archive_filename);

    println!("Downloading Zoi from: {download_url}");
    download_file(&download_url, &temp_archive_path)?;
    verify_checksum(&temp_archive_path, checksums_content, &archive_filename)?;

    extract_archive(&temp_archive_path, temp_dir.path())?;

    let binary_filename = if os == "windows" { "zoi.exe" } else { "zoi" };
    let new_binary_path = temp_dir.path().join(binary_filename);
    if !new_binary_path.exists() {
        return Err(anyhow!(
            "Could not find executable in the extracted archive."
        ));
    }
    Ok(new_binary_path)
}

pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>,
) -> Result<()> {
    let current_exe_path = env::current_exe()?;
    let path_str = current_exe_path.to_string_lossy();

    let is_cargo_install = dirs::home_dir()
        .map(|home| current_exe_path.starts_with(home.join(".cargo").join("bin")))
        .unwrap_or(false);

    let pkg_manager = if path_str.contains("/Cellar/") {
        Some("Homebrew")
    } else if path_str.contains("scoop/apps/") {
        Some("Scoop")
    } else if path_str.starts_with("/usr/bin/") {
        Some("a system package manager")
    } else if is_cargo_install {
        Some("Cargo")
    } else {
        None
    };

    if let Some(pm) = pkg_manager {
        if !force {
            eprintln!(
                "{}{}{}",
                "Warning: ".yellow().bold(),
                "It looks like Zoi was installed via ".yellow(),
                pm.yellow().bold()
            );
            eprintln!(
                "{}",
                "Using 'zoi upgrade' may conflict with your package manager.".yellow()
            );
            let upgrade_command = match pm {
                "Homebrew" => "brew upgrade zoi",
                "Scoop" => "scoop update zoi",
                "Cargo" => "cargo install zoi-rs",
                _ => "your package manager's upgrade command",
            };
            eprintln!(
                "It is recommended to use '{}' to upgrade Zoi.",
                upgrade_command.cyan()
            );
            eprintln!(
                "To override this check and proceed anyway, run with the '{}' flag.",
                "--force".cyan()
            );
            return Err(anyhow!("managed_by_package_manager"));
        } else {
            println!(
                "{}{}",
                "Warning: ".yellow().bold(),
                "Forcing self-upgrade on a package-manager-controlled installation.".yellow()
            );
        }
    }

    let current_version = if status.is_empty() || status.eq_ignore_ascii_case("stable") {
        number.to_string()
    } else {
        format!("{}-{}", number, status.to_lowercase())
    };

    let latest_tag = if let Some(tag_name) = tag {
        println!("Upgrading to specified tag: {}", tag_name.green());
        tag_name
    } else {
        let branch_prefix = if let Some(b) = custom_branch {
            println!("Upgrading to latest release from branch: {}", b.green());
            format!("{}-", b)
        } else if branch.eq_ignore_ascii_case("public") {
            "Pub-".to_string()
        } else {
            "Prod-".to_string()
        };
        get_latest_tag(&branch_prefix)?
    };

    let parts: Vec<&str> = latest_tag.split('-').collect();
    let latest_version_num = parts
        .last()
        .ok_or(anyhow!("Could not get version number from tag"))?;

    let latest_version_str = if parts.len() > 2 {
        let prerelease = parts[1].to_lowercase();
        format!("{}-{}", latest_version_num, prerelease)
    } else {
        latest_version_num.to_string()
    };

    if !force && !self_update::version::bump_is_greater(&current_version, &latest_version_str)? {
        println!(
            "
{}",
            "You are already on the latest version!".green()
        );
        return Err(anyhow!("already_on_latest"));
    }

    let (os, arch) = get_platform_info()?;

    let base_url =
        format!("https://gitlab.com/{GITLAB_PROJECT_PATH}/-/releases/{latest_tag}/downloads");
    let checksums_txt_url = format!("{base_url}/checksums.txt");

    println!(
        "Downloading archive and checksums from: {}",
        checksums_txt_url
    );
    let checksums_txt_content = reqwest::blocking::get(&checksums_txt_url)?.text()?;

    let new_binary_path = fallback_full_upgrade(&base_url, &checksums_txt_content, os, arch)?;

    println!("Replacing current executable...");
    self_replace::self_replace(&new_binary_path)?;

    Ok(())
}
