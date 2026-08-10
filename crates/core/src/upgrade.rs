use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use dirs;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use self_update::self_replace;
use serde::Deserialize;
use sha2::{Digest, Sha512};
use tar::Archive;
use tempfile::Builder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// The GitLab project path for the Zoi repository.
const GITLAB_PROJECT_PATH: &str = "zillowe/zillwen/zusty/zoi";
/// The GitLab project ID for the Zoi repository.
const GITLAB_PROJECT_ID: &str = "71087662";

/// Represents a release from the GitLab API.
#[derive(Debug, Deserialize)]
struct GitLabRelease {
    /// The tag name of the release.
    tag_name: String
}

/// Fetches the latest tag from GitLab for a given branch prefix.
fn get_latest_tag(branch_prefix: &str) -> Result<String> {
    println!("Fetching latest release information from GitLab...");
    let api_url = format!(
        "https://gitlab.com/api/v4/projects/{GITLAB_PROJECT_ID}/releases"
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("Zoi-Upgrader")
        .use_rustls_tls()
        .build()?;
    let releases: Vec<GitLabRelease> = client.get(&api_url).send()?.json()?;

    let latest_tag = releases
        .into_iter()
        .find(|r| r.tag_name.starts_with(branch_prefix))
        .map(|r| r.tag_name)
        .ok_or_else(|| {
            anyhow!("No release found with prefix '{branch_prefix}'")
        })?;

    println!(
        "Found latest tag for branch prefix '{}': {}",
        branch_prefix,
        latest_tag.green()
    );
    Ok(latest_tag)
}

/// Downloads a file from a URL to a local path with a progress bar.
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
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                 {bytes}/{total_bytes} ({bytes_per_sec})"
            )?
            .progress_chars("#>- ")
    );

    let mut dest = File::create(path)?;
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(
            buffer
                .get(..bytes_read)
                .ok_or_else(|| anyhow!("Buffer overflow during write"))?
        )?;
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Download complete.");
    Ok(())
}

/// Extracts a zip or zstd-compressed tar archive to a target directory.
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

/// Verifies the SHA-512 checksum of a file against expected content.
fn verify_checksum(
    file_path: &Path,
    checksums_content: &str,
    filename: &str
) -> Result<()> {
    println!("Verifying checksum for {filename}...");
    let expected_hash = checksums_content
        .lines()
        .find(|line| line.contains(filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or(anyhow!("Checksum not found for {filename}."))?;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = io::Read::read(&mut file, &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..bytes_read)
                .ok_or_else(|| anyhow!("Buffer overflow during hash update"))?
        );
    }
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(anyhow!(
            "Checksum mismatch for {filename}! The file may be corrupt."
        ));
    }
    println!("Checksum verified successfully for {}.", filename.green());
    Ok(())
}

/// Returns the current platform's OS and architecture labels used in release
/// filenames.
fn get_platform_info() -> Result<(&'static str, &'static str)> {
    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        _ => return Err(anyhow!("Unsupported OS: {}", env::consts::OS))
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => {
            return Err(anyhow!(
                "Unsupported architecture: {}",
                env::consts::ARCH
            ));
        }
    };
    Ok((os, arch))
}

/// Performs a full upgrade by downloading the entire binary archive.
fn fallback_full_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str
) -> Result<(PathBuf, tempfile::TempDir)> {
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
    Ok((new_binary_path, temp_dir))
}

/// Attempts a delta upgrade by downloading a bsdiff patch.
fn try_delta_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str,
    current_version: &str,
    latest_version: &str
) -> Result<(PathBuf, tempfile::TempDir)> {
    let archive_basename = format!("zoi-{os}-{arch}");
    let bsdiff_filename = format!(
        "{archive_basename}.from-v{current_version}-to-v{latest_version}.\
         bsdiff"
    );
    let download_url = format!("{base_url}/{bsdiff_filename}");

    if !checksums_content.contains(&bsdiff_filename) {
        return Err(anyhow!(
            "Delta patch not available for this upgrade path."
        ));
    }

    let temp_dir = Builder::new().prefix("zoi-delta-upgrade").tempdir()?;
    let temp_patch_path = temp_dir.path().join(&bsdiff_filename);

    println!("{} Downloading delta patch...", "::".bold().blue());
    download_file(&download_url, &temp_patch_path)?;
    verify_checksum(&temp_patch_path, checksums_content, &bsdiff_filename)?;

    println!("{} Applying delta patch...", "::".bold().blue());
    let current_exe_path = env::current_exe()?;
    let mut old_binary = Vec::new();
    File::open(&current_exe_path)?.read_to_end(&mut old_binary)?;

    let mut patch_data = Vec::new();
    File::open(&temp_patch_path)?.read_to_end(&mut patch_data)?;

    let raw_patch = if patch_data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        let mut decoder = ZstdDecoder::new(std::io::Cursor::new(&patch_data))?;
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        buf
    } else {
        patch_data
    };

    let mut cursor = std::io::Cursor::new(raw_patch);
    let mut new_binary = Vec::new();
    bsdiff::patch(&old_binary, &mut cursor, &mut new_binary)?;

    let binary_filename = if os == "windows" { "zoi.exe" } else { "zoi" };
    let new_binary_path = temp_dir.path().join(binary_filename);
    std::fs::write(&new_binary_path, &new_binary)?;

    Ok((new_binary_path, temp_dir))
}

/// Runs the upgrade process for Zoi.
///
/// # Errors
///
/// Returns an error if:
/// - Zoi is in offline mode.
/// - The GitLab API cannot be reached.
/// - The download fails or the checksum is invalid.
/// - The binary replacement fails.
pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>
) -> Result<()> {
    if crate::offline::is_offline() {
        return Err(anyhow!("Cannot upgrade Zoi: Zoi is in offline mode."));
    }
    let current_exe_path = env::current_exe()?;
    let path_str = current_exe_path.to_string_lossy();

    let is_cargo_install = dirs::home_dir().is_some_and(|home| {
        current_exe_path.starts_with(home.join(".cargo").join("bin"))
    });

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
                "Using 'zoi upgrade' may conflict with your package manager."
                    .yellow()
            );
            let upgrade_command = match pm {
                "Homebrew" => "brew upgrade zoi",
                "Scoop" => "scoop update zoi",
                "Cargo" => "cargo install zoi-rs",
                _ => "your package manager's upgrade command"
            };
            eprintln!(
                "It is recommended to use '{}' to upgrade Zoi.",
                upgrade_command.cyan()
            );
            eprintln!(
                "To override this check and proceed anyway, run with the '{}' \
                 flag.",
                "--force".cyan()
            );
            return Err(anyhow!("managed_by_package_manager"));
        }

        println!(
            "{}{}",
            "Warning: ".yellow().bold(),
            "Forcing self-upgrade on a package-manager-controlled \
             installation."
                .yellow()
        );
    }

    let current_version = if status.is_empty()
        || status.eq_ignore_ascii_case("stable")
        || status.eq_ignore_ascii_case("release")
    {
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
            format!("{b}-")
        } else if branch.eq_ignore_ascii_case("public") {
            "Pub-".to_string()
        } else {
            "Prod-".to_string()
        };
        get_latest_tag(&branch_prefix)?
    };

    let parts: Vec<&str> = latest_tag.split('-').collect();
    let latest_version_num = if parts.len() >= 3 {
        parts
            .get(2)
            .copied()
            .ok_or_else(|| anyhow!("Missing version number in tag parts"))?
    } else {
        parts
            .last()
            .ok_or(anyhow!("Could not get version number from tag"))?
    };

    let latest_version_str = if parts.len() >= 3 {
        let prerelease = parts
            .get(1)
            .copied()
            .ok_or_else(|| anyhow!("Missing prerelease label in tag parts"))?
            .to_lowercase();
        if prerelease == "release" || prerelease == "stable" {
            latest_version_num.to_string()
        } else {
            format!("{latest_version_num}-{prerelease}")
        }
    } else {
        latest_version_num.to_string()
    };

    if !force
        && !self_update::version::bump_is_greater(
            &current_version,
            &latest_version_str
        )?
    {
        println!(
            "
{}",
            "You are already on the latest version!".green()
        );
        return Err(anyhow!("already_on_latest"));
    }

    let (os, arch) = get_platform_info()?;

    let base_url = format!(
        "https://gitlab.com/{GITLAB_PROJECT_PATH}/-/releases/{latest_tag}/downloads"
    );
    let checksums_txt_url = format!("{base_url}/checksums.txt");

    println!("Downloading archive and checksums from: {checksums_txt_url}");
    let checksums_txt_content =
        reqwest::blocking::get(&checksums_txt_url)?.text()?;

    let (new_binary_path, _temp_dir_guard) = if force {
        fallback_full_upgrade(&base_url, &checksums_txt_content, os, arch)?
    } else {
        match try_delta_upgrade(
            &base_url,
            &checksums_txt_content,
            os,
            arch,
            &current_version,
            &latest_version_str
        ) {
            Ok(res) => res,
            Err(e) => {
                println!(
                    "Delta upgrade failed: {e}. Falling back to full upgrade."
                );
                fallback_full_upgrade(
                    &base_url,
                    &checksums_txt_content,
                    os,
                    arch
                )?
            }
        }
    };

    println!("Replacing current executable...");
    self_replace::self_replace(&new_binary_path)?;

    Ok(())
}
