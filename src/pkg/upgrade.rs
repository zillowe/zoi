use bsdiff;
use colored::*;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use self_update::self_replace;
use serde::Deserialize;
use sha2::{Digest, Sha512};

use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

const GITLAB_PROJECT_PATH: &str = "Zillowe/Zillwen/Zusty/Zoi";

#[derive(Debug, Deserialize)]
struct GitLabRelease {
    tag_name: String,
}

fn get_latest_tag(is_dev_build: bool) -> Result<String, Box<dyn Error>> {
    println!("Fetching latest release information from GitLab...");
    let api_url = format!(
        "https://gitlab.com/api/v4/projects/{}/releases",
        GITLAB_PROJECT_PATH.replace('/', "%2F")
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("Zoi-Upgrader")
        .build()?;
    let releases: Vec<GitLabRelease> = client.get(&api_url).send()?.json()?;

    let tag_prefix = if is_dev_build { "Dev-" } else { "Prod-" };
    let latest_tag = releases
        .into_iter()
        .find(|r| r.tag_name.starts_with(tag_prefix))
        .map(|r| r.tag_name)
        .ok_or_else(|| format!("No release found with prefix '{tag_prefix}'"))?;

    println!("Found latest tag: {}", latest_tag.green());
    Ok(latest_tag)
}

fn download_file(url: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

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

fn download_patch_file(url: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Zoi-Upgrader")
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()?;
    let mut response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

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

fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), Box<dyn Error>> {
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

fn verify_checksum(
    file_path: &Path,
    checksums_content: &str,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Verifying checksum for {}...", filename);
    let expected_hash = checksums_content
        .lines()
        .find(|line| line.contains(filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or(format!("Checksum not found for {}.", filename))?;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    io::copy(&mut file, &mut hasher)?;
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(format!(
            "Checksum mismatch for {}! The file may be corrupt.",
            filename
        )
        .into());
    }
    println!("Checksum verified successfully for {}.", filename.green());
    Ok(())
}

fn get_platform_info() -> Result<(&'static str, &'static str), Box<dyn Error>> {
    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        _ => return Err(format!("Unsupported OS: {}", env::consts::OS).into()),
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(format!("Unsupported architecture: {}", env::consts::ARCH).into()),
    };
    Ok((os, arch))
}

fn attempt_patch_upgrade(
    base_url: &str,
    bin_checksums: &str,
    patch_checksums: &str,
    archive_filename: &str,
    patch_filename: &str,
    new_binary_checksum_name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    println!("\nAttempting patch-based upgrade (bsdiff)...");
    let temp_dir = Builder::new().prefix("zoi-patch-upgrade").tempdir()?;

    let old_archive_url = format!("{}/{}", base_url, archive_filename);
    let old_archive_path = temp_dir.path().join(archive_filename);
    println!("Downloading base archive from: {}", old_archive_url);
    download_file(&old_archive_url, &old_archive_path)?;

    let patch_url = format!("{}/{}", base_url, patch_filename);
    let patch_path = temp_dir.path().join(patch_filename);
    println!("Downloading patch from: {}", patch_url);
    download_patch_file(&patch_url, &patch_path)?;
    verify_checksum(&patch_path, patch_checksums, patch_filename)?;

    println!("Extracting old archive to obtain base binary...");
    let old_extract_dir = temp_dir.path().join("old");
    fs::create_dir_all(&old_extract_dir)?;
    extract_archive(&old_archive_path, &old_extract_dir)?;

    let old_binary_name = if cfg!(target_os = "windows") {
        "zoi.exe"
    } else {
        "zoi"
    };
    let old_binary_path = old_extract_dir.join(old_binary_name);
    if !old_binary_path.exists() {
        return Err("Could not find old binary inside the previous archive.".into());
    }

    println!("Applying patch to derive new binary...");
    let old_data = fs::read(&old_binary_path)?;
    let patch_data = fs::read(&patch_path)?;
    let mut new_binary_data = Vec::new();
    let mut patch_reader = Cursor::new(patch_data);
    bsdiff::patch(&old_data, &mut patch_reader, &mut new_binary_data)?;

    let new_binary_temp_path = temp_dir.path().join(if cfg!(target_os = "windows") {
        "zoi.exe"
    } else {
        "zoi"
    });
    fs::write(&new_binary_temp_path, &new_binary_data)?;

    println!("Verifying patched binary...");
    verify_checksum(
        &new_binary_temp_path,
        bin_checksums,
        new_binary_checksum_name,
    )?;

    Ok(new_binary_temp_path)
}

fn fallback_full_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    println!("\nFalling back to full binary download...");
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
        return Err("Could not find executable in the extracted archive.".into());
    }
    Ok(new_binary_path)
}

pub fn run(branch: &str, status: &str, number: &str) -> Result<(), Box<dyn Error>> {
    let current_version = if status.is_empty() || status.eq_ignore_ascii_case("stable") {
        number.to_string()
    } else {
        format!("{}-{}", number, status.to_lowercase())
    };

    let is_dev_build = branch.eq_ignore_ascii_case("development");
    let latest_tag = get_latest_tag(is_dev_build)?;

    let parts: Vec<&str> = latest_tag.split('-').collect();
    let latest_version_num = parts
        .last()
        .ok_or("Could not get version number from tag")?;

    let latest_version_str = if parts.len() > 2 {
        let prerelease = parts[1].to_lowercase();
        format!("{}-{}", latest_version_num, prerelease)
    } else {
        latest_version_num.to_string()
    };

    if !self_update::version::bump_is_greater(&current_version, &latest_version_str)? {
        println!("\n{}", "You are already on the latest version!".green());
        return Err("already_on_latest".into());
    }

    let (os, arch) = get_platform_info()?;
    let patch_filename = if os == "windows" {
        format!("zoi-{}-{}.exe.patch", os, arch)
    } else {
        format!("zoi-{}-{}.patch", os, arch)
    };

    let base_url =
        format!("https://gitlab.com/{GITLAB_PROJECT_PATH}/-/releases/{latest_tag}/downloads");
    let checksums_bin_url = format!("{base_url}/checksums-bin.txt");
    let checksums_txt_url = format!("{base_url}/checksums.txt");

    println!("Downloading binary checksums from: {}", checksums_bin_url);
    let checksums_bin_content = reqwest::blocking::get(&checksums_bin_url)?.text()?;

    println!(
        "Downloading archive and patch checksums from: {}",
        checksums_txt_url
    );
    let checksums_txt_content = reqwest::blocking::get(&checksums_txt_url)?.text()?;

    let new_binary_checksum_name = format!("zoi-{}-{}-v{}", os, arch, latest_version_str);

    let archive_ext = if os == "windows" { "zip" } else { "tar.zst" };
    let archive_filename = format!("zoi-{os}-{arch}.{archive_ext}");

    let new_binary_path = match attempt_patch_upgrade(
        &base_url,
        &checksums_bin_content,
        &checksums_txt_content,
        &archive_filename,
        &patch_filename,
        &new_binary_checksum_name,
    ) {
        Ok(path) => {
            println!("{}", "Patch upgrade successful!".green());
            path
        }
        Err(e) => {
            println!(
                "{}: {}. {}",
                "Patch upgrade failed".yellow(),
                e,
                "Attempting full download.".yellow()
            );
            fallback_full_upgrade(&base_url, &checksums_txt_content, os, arch)?
        }
    };

    println!("Replacing current executable...");
    self_replace::self_replace(&new_binary_path)?;

    Ok(())
}
