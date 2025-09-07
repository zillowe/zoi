use super::structs::FinalMetadata;
use crate::utils;
use colored::*;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha512};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;
use tempfile::Builder;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

fn download_file(url: &str, pb: &ProgressBar) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    pb.set_length(total_size);

    let mut downloaded_bytes = Vec::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Downloaded.");
    Ok(downloaded_bytes)
}

fn verify_checksum(data: &[u8], expected_checksum: &str) -> Result<(), Box<dyn Error>> {
    println!("Verifying checksum...");
    let mut hasher = Sha512::new();
    hasher.update(data);
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash.eq_ignore_ascii_case(expected_checksum) {
        println!("{}", "Checksum verified successfully.".green());
        Ok(())
    } else {
        Err(format!(
            "Checksum mismatch!\n  Expected: {}\n  Actual: {}",
            expected_checksum, actual_hash
        )
        .into())
    }
}

pub fn run(meta_file: &Path) -> Result<(), Box<dyn Error>> {
    println!("Building package from: {}", meta_file.display());

    let content = fs::read_to_string(meta_file)?;
    let metadata: FinalMetadata = serde_json::from_str(&content)?;

    let current_platform = utils::get_platform()?;
    println!("Building for current platform: {}", current_platform);

    let asset = metadata
        .installation
        .assets
        .iter()
        .find(|a| a.platform == current_platform)
        .ok_or_else(|| format!("No asset found for platform '{}'", current_platform))?;

    println!("Found asset for platform: {}", asset.url);

    let build_dir = Builder::new().prefix("zoi-build-").tempdir()?;
    println!("Using build directory: {}", build_dir.path().display());

    let pb = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
            )?
            .progress_chars("#>-"),
    );

    let downloaded_data = download_file(&asset.url, &pb)?;

    if let Some(checksum) = &asset.checksum {
        verify_checksum(&downloaded_data, checksum)?;
    } else {
        println!(
            "{}",
            "No checksum provided, skipping verification.".yellow()
        );
    }

    let staging_dir = build_dir.path().join("staging");
    fs::create_dir_all(&staging_dir)?;

    if let Some(man_url) = &metadata.man_url {
        println!("Downloading manual from {}", man_url);
        let man_pb = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] Downloading manual...")?,
        );
        let man_data = download_file(man_url, &man_pb)?;
        let man_filename = if man_url.ends_with(".md") {
            "man.md"
        } else {
            "man.txt"
        };
        fs::write(staging_dir.join(man_filename), man_data)?;
    }

    let data_dir = staging_dir.join("data");
    fs::create_dir_all(&data_dir)?;

    if metadata.installation.install_type == "com_binary" {
        println!("Extracting compressed binary...");
        let archive_cursor = Cursor::new(downloaded_data);
        let filename = Path::new(&asset.url)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        match filename {
            "zip" => ZipArchive::new(archive_cursor)?.extract(&data_dir)?,
            "zst" => Archive::new(ZstdDecoder::new(archive_cursor)?).unpack(&data_dir)?,
            "gz" => Archive::new(GzDecoder::new(archive_cursor)).unpack(&data_dir)?,
            "xz" => Archive::new(XzDecoder::new(archive_cursor)).unpack(&data_dir)?,
            _ => return Err("Unsupported archive format".into()),
        }
    } else if metadata.installation.install_type == "binary" {
        let bin_dir = data_dir.join("usr/bin");
        fs::create_dir_all(&bin_dir)?;
        let bin_name = metadata
            .installation
            .binary_path
            .as_ref()
            .unwrap_or(&metadata.name);
        fs::write(bin_dir.join(bin_name), downloaded_data)?;
    } else {
        return Err(format!(
            "Unsupported install type for build: {}",
            metadata.installation.install_type
        )
        .into());
    }

    println!("Packaging final archive...");
    fs::copy(meta_file, staging_dir.join("metadata.json"))?;

    let output_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        metadata.name, metadata.version, asset.platform
    );
    let output_path = meta_file.with_file_name(output_filename);

    let file = File::create(&output_path)?;
    let encoder = zstd::stream::write::Encoder::new(file, 0)?.auto_finish();
    let mut tar_builder = tar::Builder::new(encoder);
    tar_builder.append_dir_all(".", &staging_dir)?;
    tar_builder.finish()?;

    println!(
        "{}",
        format!("Successfully built package: {}", output_path.display()).green()
    );

    Ok(())
}
