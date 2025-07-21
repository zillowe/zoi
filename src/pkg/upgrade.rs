use colored::*;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use self_update::{cargo_crate_version, self_replace};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use tar::Archive;
use tempfile::Builder;
use xz2::read::XzDecoder;
use zip::ZipArchive;

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

fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), Box<dyn Error>> {
    println!("Extracting binary...");
    let file = File::open(archive_path)?;

    if archive_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        let mut archive = ZipArchive::new(file)?;
        archive.extract(target_dir)?;
    } else {
        let tar = XzDecoder::new(file);
        let mut archive = Archive::new(tar);
        archive.unpack(target_dir)?;
    }
    Ok(())
}

fn verify_checksum(
    archive_path: &Path,
    checksums_content: &str,
    archive_filename: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Verifying checksum...");
    let expected_hash = checksums_content
        .lines()
        .find(|line| line.contains(archive_filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or("Checksum not found for the archive.")?;

    let mut file = File::open(archive_path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err("Checksum mismatch! The downloaded file may be corrupt.".into());
    }
    println!("{}", "Checksum verified successfully.".green());
    Ok(())
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let current_version = cargo_crate_version!();
    let is_dev_build = current_version.contains("-dev");
    let latest_tag = get_latest_tag(is_dev_build)?;
    let latest_version_str = latest_tag.trim_start_matches(|c: char| !c.is_numeric());

    if !self_update::version::bump_is_greater(current_version, latest_version_str)? {
        println!("\n{}", "You are already on the latest version!".green());
        return Ok(());
    }

    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        _ => return Err(format!("Unsupported OS: {}", env::consts::OS).into()),
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(format!("Unsupported architecture: {}", env::consts::ARCH).into()),
    };

    let archive_ext = if os == "windows" { "zip" } else { "tar.xz" };
    let archive_filename = format!("zoi-{os}-{arch}.{archive_ext}");
    let base_url = format!(
        "https://gitlab.com/{GITLAB_PROJECT_PATH}/-/releases/{latest_tag}/downloads"
    );
    let download_url = format!("{base_url}/{archive_filename}");
    let checksums_url = format!("{base_url}/checksums.txt");

    let temp_dir = Builder::new().prefix("zoi-upgrade").tempdir()?;
    let temp_archive_path = temp_dir.path().join(&archive_filename);

    println!(
        "Downloading Zoi v{latest_version_str} from: {download_url}"
    );
    download_file(&download_url, &temp_archive_path)?;

    let checksums_content = reqwest::blocking::get(&checksums_url)?.text()?;
    verify_checksum(&temp_archive_path, &checksums_content, &archive_filename)?;

    extract_archive(&temp_archive_path, temp_dir.path())?;

    let new_binary_path = temp_dir
        .path()
        .join(if os == "windows" { "zoi.exe" } else { "zoi" });
    if !new_binary_path.exists() {
        return Err("Could not find executable in the extracted archive.".into());
    }

    self_replace::self_replace(&new_binary_path)?;

    Ok(())
}
