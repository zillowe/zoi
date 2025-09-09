use super::structs::FinalMetadata;
use crate::{pkg, utils};
use colored::*;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use sequoia_openpgp::{
    KeyHandle,
    cert::Cert,
    parse::{
        Parse,
        stream::{DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper},
    },
    policy::StandardPolicy,
};
use sha2::{Digest, Sha512};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;
use tempfile::Builder;
use tokio::runtime::Runtime;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    fn new(f: F) -> Defer<F> {
        Defer { f: Some(f) }
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}

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

struct Helper {
    certs: Vec<Cert>,
}

impl VerificationHelper for Helper {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        let matching_certs: Vec<Cert> = self
            .certs
            .iter()
            .filter(|cert| {
                ids.iter().any(|id| {
                    cert.keys().any(|key| match *id {
                        KeyHandle::KeyID(ref keyid) => key.key().keyid() == *keyid,
                        KeyHandle::Fingerprint(ref fp) => key.key().fingerprint() == *fp,
                    })
                })
            })
            .cloned()
            .collect();
        Ok(matching_certs)
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(|r| r.is_ok()) {
                        return Ok(());
                    } else {
                        return Err(anyhow::anyhow!("No valid signature found"));
                    }
                }
                _ => return Err(anyhow::anyhow!("Unexpected message structure")),
            }
        }
        Err(anyhow::anyhow!("No signature layer found"))
    }
}

fn verify_signature(
    data: &[u8],
    signature_url: &str,
    metadata: &FinalMetadata,
) -> Result<(), Box<dyn Error>> {
    println!("Verifying signature for downloaded asset...");

    let mut keys_to_check: Vec<(&str, Option<&str>)> = Vec::new();
    if let Some(key) = metadata.maintainer.key.as_deref() {
        keys_to_check.push((key, metadata.maintainer.key_name.as_deref()));
    }
    if let Some(author) = &metadata.author
        && let Some(key) = author.key.as_deref()
    {
        keys_to_check.push((key, author.key_name.as_deref()));
    }

    if keys_to_check.is_empty() {
        println!(
            "{} Signature URL found, but no maintainer or author key is defined. Skipping verification.",
            "Warning:".yellow(),
        );
        return Ok(());
    }

    let rt = Runtime::new()?;
    rt.block_on(async {
        let local_keys = pkg::pgp::get_all_local_keys_info().unwrap_or_default();
        let mut certs: Vec<Cert> = local_keys.iter().map(|ki| ki.cert.clone()).collect();
        let local_key_names: Vec<String> = local_keys.iter().map(|ki| ki.name.clone()).collect();
        let local_fingerprints: Vec<String> = certs
            .iter()
            .map(|c| c.fingerprint().to_string().to_uppercase())
            .collect();

        for (key_source, key_name) in &keys_to_check {
            let mut key_found_locally = false;

            if let Some(name) = key_name
                && local_key_names.iter().any(|n| n == name)
            {
                key_found_locally = true;
            }

            if !key_found_locally
                && key_source.len() == 40
                && key_source.chars().all(|c| c.is_ascii_hexdigit())
                && local_fingerprints
                    .iter()
                    .any(|fp| fp == &key_source.to_uppercase())
            {
                key_found_locally = true;
            }

            if key_found_locally {
                println!(
                    "Found key for '{}' locally.",
                    key_name.unwrap_or(key_source)
                );
                continue;
            }

            println!(
                "Key for '{}' not found locally, attempting to import...",
                key_name.unwrap_or(key_source)
            );

            let key_bytes_result = if key_source.starts_with("http") {
                println!("Importing key from URL: {}", key_source.cyan());
                reqwest::get(*key_source).await?.bytes().await
            } else if key_source.len() == 40 && key_source.chars().all(|c| c.is_ascii_hexdigit()) {
                let fingerprint = key_source.to_uppercase();
                let key_server_url = format!(
                    "https://keys.openpgp.org/vks/v1/by-fingerprint/{}",
                    fingerprint
                );
                println!(
                    "Importing key for fingerprint {} from keyserver...",
                    fingerprint.cyan()
                );
                reqwest::get(&key_server_url).await?.bytes().await
            } else {
                println!(
                    "{} Invalid key source: '{}'. Must be a URL or a 40-character GPG fingerprint.",
                    "Warning:".yellow(),
                    key_source
                );
                continue;
            };

            match key_bytes_result {
                Ok(key_bytes) => {
                    if let Ok(cert) = Cert::from_bytes(&key_bytes) {
                        if let Some(name) = key_name
                            && let Err(e) = pkg::pgp::add_key_from_bytes(&key_bytes, name)
                        {
                            println!(
                                "{} Failed to save imported key '{}': {}",
                                "Warning:".yellow(),
                                name,
                                e
                            );
                        }
                        certs.push(cert);
                    } else {
                        println!(
                            "{} Failed to parse certificate from source: {}",
                            "Warning:".yellow(),
                            key_source
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "{} Failed to download key from source {}: {}",
                        "Warning:".yellow(),
                        key_source,
                        e
                    );
                }
            }
        }

        if certs.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid public keys found to verify signature."
            ));
        }

        println!("Downloading signature from: {}", signature_url);
        let sig_bytes = reqwest::get(signature_url).await?.bytes().await?;

        let policy = &StandardPolicy::new();
        let helper = Helper { certs };

        let mut verifier =
            DetachedVerifierBuilder::from_bytes(&sig_bytes)?.with_policy(policy, None, helper)?;

        verifier.verify_bytes(data)?;

        println!("{}", "Signature verified successfully.".green());
        Ok(())
    })?;

    Ok(())
}

fn build_for_platform(
    meta_file: &Path,
    metadata: &FinalMetadata,
    platform: &str,
) -> Result<(), Box<dyn Error>> {
    println!("--- Building for platform: {} ---", platform.cyan());

    let build_dir = Builder::new()
        .prefix(&format!("zoi-build-{}-", platform))
        .tempdir()?;
    println!("Using build directory: {}", build_dir.path().display());

    let staging_dir = build_dir.path().join("staging");
    fs::create_dir_all(&staging_dir)?;
    let data_dir = staging_dir.join("data");
    fs::create_dir_all(&data_dir)?;

    if metadata.installation.install_type == "source" {
        let inst = &metadata.installation;

        if let Some(docker_image_str) = &inst.docker_image {
            let image_to_pull = if let Some(image) = docker_image_str.strip_prefix("ghcr:") {
                format!("ghcr.io/{}", image)
            } else if let Some(image) = docker_image_str.strip_prefix("hub:") {
                image.to_string()
            } else {
                docker_image_str.clone()
            };

            println!("Building using Docker image: {}", image_to_pull.cyan());
            if !utils::command_exists("docker") {
                return Err("Docker command not found. Please install Docker.".into());
            }

            println!("Pulling Docker image...");
            let output = std::process::Command::new("docker")
                .arg("pull")
                .arg(&image_to_pull)
                .output()?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                return Err(format!("Failed to pull Docker image '{}'", image_to_pull).into());
            }

            let container_name = format!("zoi-build-{}-{}", metadata.name, platform);
            let _ = std::process::Command::new("docker")
                .arg("rm")
                .arg("-f")
                .arg(&container_name)
                .output()?;

            println!("Starting Docker container...");
            let output = std::process::Command::new("docker")
                .arg("run")
                .arg("-d")
                .arg("--name")
                .arg(&container_name)
                .arg(&image_to_pull)
                .arg("tail")
                .arg("-f")
                .arg("/dev/null")
                .output()?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                return Err("Failed to start Docker container.".into());
            }

            // Defer container cleanup
            let _cleanup = Defer::new(|| {
                println!("Stopping and removing container...");
                let _ = std::process::Command::new("docker")
                    .arg("stop")
                    .arg(&container_name)
                    .output();
                let _ = std::process::Command::new("docker")
                    .arg("rm")
                    .arg(&container_name)
                    .output();
            });

            let git_url = inst
                .git
                .as_ref()
                .ok_or("Missing git url for source build")?;
            let repo_path_in_container = "/build";

            println!("Cloning from {} into container...", git_url);
            let mut git_cmd_args = vec![
                "exec".to_string(),
                container_name.clone(),
                "git".to_string(),
                "clone".to_string(),
            ];
            if let Some(branch) = &inst.branch {
                git_cmd_args.push("-b".to_string());
                git_cmd_args.push(branch.clone());
            }
            git_cmd_args.push(git_url.clone());
            git_cmd_args.push(repo_path_in_container.to_string());

            let output = std::process::Command::new("docker")
                .args(git_cmd_args)
                .output()?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                return Err("Failed to clone source repository inside container.".into());
            }

            let cloned_repo_name = git_url
                .split('/')
                .next_back()
                .unwrap()
                .strip_suffix(".git")
                .unwrap_or_else(|| git_url.split('/').next_back().unwrap());
            let workdir_in_container = format!("{}/{}", repo_path_in_container, cloned_repo_name);

            if let Some(tag) = &inst.tag {
                let output = std::process::Command::new("docker")
                    .arg("exec")
                    .arg(&container_name)
                    .arg("git")
                    .arg("-C")
                    .arg(&workdir_in_container)
                    .arg("checkout")
                    .arg(tag)
                    .output()?;
                if !output.status.success() {
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                    return Err(format!("Failed to checkout tag '{}' in container", tag).into());
                }
            }

            let os_part = platform.split('-').next().unwrap_or(platform);
            let commands = inst
                .build_commands
                .as_ref()
                .and_then(|m| m.get(os_part))
                .ok_or(format!("No build commands found for platform {}", platform))?;

            println!("{}", "Running build commands in container...".bold());
            for cmd_str in commands {
                println!("Executing in container: {}", cmd_str.cyan());
                let output = std::process::Command::new("docker")
                    .arg("exec")
                    .arg("-w")
                    .arg(&workdir_in_container)
                    .arg(&container_name)
                    .arg("bash")
                    .arg("-c")
                    .arg(cmd_str)
                    .output()?;
                if !output.status.success() {
                    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                    return Err(format!("Build command failed in container: '{}'", cmd_str).into());
                }
            }

            let bin_path_str = inst
                .binary_path
                .as_ref()
                .and_then(|m| m.get(os_part))
                .ok_or(format!("No binary_path found for platform {}", platform))?;

            let built_binary_path_in_container =
                Path::new(&workdir_in_container).join(bin_path_str);

            let temp_bin_dir = build_dir.path().join("bin");
            fs::create_dir_all(&temp_bin_dir)?;
            let local_bin_path =
                temp_bin_dir.join(built_binary_path_in_container.file_name().unwrap());

            println!("Copying built binary from container...");
            let cp_arg = format!(
                "{}:{}",
                &container_name,
                built_binary_path_in_container.to_str().unwrap()
            );
            let output = std::process::Command::new("docker")
                .arg("cp")
                .arg(&cp_arg)
                .arg(&local_bin_path)
                .output()?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to copy binary from container: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
                .into());
            }

            let bin_dir = data_dir.join("usr/bin");
            fs::create_dir_all(&bin_dir)?;
            let dest_bin_path = bin_dir.join(local_bin_path.file_name().unwrap());
            fs::copy(local_bin_path, dest_bin_path)?;
        } else {
            let inst = &metadata.installation;
            let git_url = inst
                .git
                .as_ref()
                .ok_or("Missing git url for source build")?;

            let git_path = build_dir.path().join("git");
            println!("Cloning from {}...", git_url);
            let mut git_cmd = std::process::Command::new("git");
            git_cmd.arg("clone");
            if let Some(branch) = &inst.branch {
                git_cmd.arg("-b").arg(branch);
            }
            git_cmd.arg(git_url).arg(&git_path);
            let output = git_cmd.output()?;

            if !output.status.success() {
                return Err("Failed to clone source repository.".into());
            }

            if let Some(tag) = &inst.tag {
                let output = std::process::Command::new("git")
                    .current_dir(&git_path)
                    .arg("checkout")
                    .arg(tag)
                    .output()?;
                if !output.status.success() {
                    return Err(format!("Failed to checkout tag '{}'", tag).into());
                }
            }

            let os_part = platform.split('-').next().unwrap_or(platform);

            let commands = inst
                .build_commands
                .as_ref()
                .and_then(|m| m.get(os_part))
                .ok_or(format!("No build commands found for platform {}", platform))?;

            println!("{}", "Running build commands...".bold());
            for cmd_str in commands {
                println!("Executing: {}", cmd_str.cyan());
                let output = if platform.starts_with("windows") {
                    std::process::Command::new("pwsh")
                        .arg("-Command")
                        .arg(cmd_str)
                        .current_dir(&git_path)
                        .output()?
                } else {
                    std::process::Command::new("bash")
                        .arg("-c")
                        .arg(cmd_str)
                        .current_dir(&git_path)
                        .output()?
                };
                if !output.status.success() {
                    return Err(format!("Build command failed: '{}'", cmd_str).into());
                }
            }

            let bin_path_str = inst
                .binary_path
                .as_ref()
                .and_then(|m| m.get(os_part))
                .ok_or(format!("No binary_path found for platform {}", platform))?;

            let built_binary_path = git_path.join(bin_path_str);
            if !built_binary_path.exists() {
                return Err(format!(
                    "Could not find built binary at specified path: {}",
                    built_binary_path.display()
                )
                .into());
            }
            let bin_dir = data_dir.join("usr/bin");
            fs::create_dir_all(&bin_dir)?;
            let dest_bin_path = bin_dir.join(built_binary_path.file_name().unwrap());
            fs::copy(built_binary_path, dest_bin_path)?;
        }
    } else {
        let asset = metadata
            .installation
            .assets
            .iter()
            .find(|a| platform.starts_with(&a.platform))
            .ok_or_else(|| format!("No asset found for platform '{}'", platform))?;

        let pb = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})"
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

        if let Some(signature_url) = &asset.signature_url {
            verify_signature(&downloaded_data, signature_url, metadata)?;
        } else {
            println!(
                "{}",
                "No signature provided, skipping verification.".yellow()
            );
        }

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
            let bin_name = metadata.name.clone();
            fs::write(bin_dir.join(bin_name), downloaded_data)?;
        } else {
            return Err(format!(
                "Unsupported install type for build: {}",
                metadata.installation.install_type
            )
            .into());
        }
    }

    let source_base_path = if metadata.installation.install_type == "source" {
        build_dir.path().join("git")
    } else {
        data_dir.clone()
    };

    if let Some(file_groups) = &metadata.installation.files {
        println!("Staging additional files for packaging...");
        let files_staging_dir = staging_dir.join("files");
        fs::create_dir_all(&files_staging_dir)?;

        for group in file_groups {
            if utils::is_platform_compatible(platform, &group.platforms) {
                for file_copy in &group.files {
                    let source_path = source_base_path.join(&file_copy.source);
                    let dest_in_archive = files_staging_dir.join(&file_copy.source);

                    if !source_path.exists() {
                        eprintln!(
                            "{} Source path for file copy does not exist, skipping: {}",
                            "Warning:".yellow(),
                            source_path.display()
                        );
                        continue;
                    }

                    if let Some(parent) = dest_in_archive.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if source_path.is_dir() {
                        for entry in WalkDir::new(&source_path)
                            .into_iter()
                            .filter_map(|e| e.ok())
                        {
                            let target_path =
                                dest_in_archive.join(entry.path().strip_prefix(&source_path)?);
                            if entry.file_type().is_dir() {
                                fs::create_dir_all(&target_path)?;
                            } else {
                                if let Some(p) = target_path.parent() {
                                    fs::create_dir_all(p)?;
                                }
                                fs::copy(entry.path(), &target_path)?;
                            }
                        }
                    } else {
                        fs::copy(&source_path, &dest_in_archive)?;
                    }
                    println!("Staged '{}' for packaging.", file_copy.source.cyan());
                }
            }
        }
    }

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

    println!("Packaging final archive...");
    fs::copy(meta_file, staging_dir.join("metadata.json"))?;

    let output_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        metadata.name, metadata.version, platform
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

pub fn run(meta_file: &Path, platforms: &[String]) -> Result<(), Box<dyn Error>> {
    println!("Building package from: {}", meta_file.display());

    let content = fs::read_to_string(meta_file)?;
    let metadata: FinalMetadata = serde_json::from_str(&content)?;

    let available_platforms: Vec<String> = if metadata.installation.install_type == "source" {
        if let Some(bc) = &metadata.installation.build_commands {
            bc.keys().cloned().collect()
        } else if let Some(bp) = &metadata.installation.binary_path {
            bp.keys().cloned().collect()
        } else {
            Vec::new()
        }
    } else {
        metadata
            .installation
            .assets
            .iter()
            .map(|a| a.platform.clone())
            .collect()
    };

    let mut target_platforms = std::collections::HashSet::new();

    if platforms.contains(&"all".to_string()) {
        if metadata.installation.install_type == "source" {
            let current_arch = utils::get_platform()?
                .split('-')
                .nth(1)
                .unwrap()
                .to_string();
            for os in available_platforms {
                target_platforms.insert(format!("{}-{}", os, &current_arch));
            }
        } else {
            for p in available_platforms {
                target_platforms.insert(p);
            }
        }
    } else {
        for p_req in platforms {
            if p_req == "current" {
                target_platforms.insert(utils::get_platform()?);
                continue;
            }
            let mut found = false;
            if metadata.installation.install_type == "source" {
                let p_req_os = p_req.split('-').next().unwrap_or(p_req);
                if available_platforms.contains(&p_req_os.to_string()) {
                    target_platforms.insert(p_req.clone());
                    found = true;
                }
            } else {
                for ap in &available_platforms {
                    if ap.starts_with(p_req) {
                        target_platforms.insert(ap.clone());
                        found = true;
                    }
                }
            }

            if !found {
                println!(
                    "{}",
                    format!(
                        "Warning: platform request '{}' did not match any available platforms.",
                        p_req
                    )
                    .yellow()
                );
            }
        }
    }

    if target_platforms.is_empty() {
        return Err("No platforms selected or available to build.".into());
    }

    println!(
        "Will build for the following platforms: {}",
        target_platforms
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
            .cyan()
    );

    for platform in target_platforms {
        if let Err(e) = build_for_platform(meta_file, &metadata, &platform) {
            eprintln!(
                "{}: Failed to build for platform {}: {}",
                "Error".red().bold(),
                platform.red(),
                e
            );
        }
    }

    Ok(())
}
