use crate::pkg::{cache, local, resolve, types};
use crate::utils;
use colored::*;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

fn get_filename_from_url(url: &str) -> &str {
    url.split('/').next_back().unwrap_or("")
}

fn get_expected_checksum(
    checksums: &types::Checksums,
    file_to_verify: &str,
    pkg: &types::Package,
    platform: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    match checksums {
        types::Checksums::Url(url) => {
            let mut url = url.replace("{version}", pkg.version.as_deref().unwrap_or(""));
            url = url.replace("{name}", &pkg.name);
            url = url.replace("{platform}", platform);

            println!("Downloading checksums from: {}", url.cyan());
            let response = reqwest::blocking::get(&url)?.text()?;
            for line in response.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 && parts[1] == file_to_verify {
                    return Ok(Some(parts[0].to_string()));
                }
            }
            if response.lines().count() == 1 && response.split_whitespace().count() == 1 {
                return Ok(Some(response.trim().to_string()));
            }
            Ok(None)
        }
        types::Checksums::List {
            checksum_type,
            items,
        } => {
            for item in items {
                let mut file_pattern = item
                    .file
                    .replace("{version}", pkg.version.as_deref().unwrap_or(""));
                file_pattern = file_pattern.replace("{name}", &pkg.name);
                file_pattern = file_pattern.replace("{platform}", platform);

                if file_pattern == file_to_verify {
                    if item.checksum.starts_with("http") {
                        println!("Downloading checksum from: {}", item.checksum.cyan());
                        let response = reqwest::blocking::get(&item.checksum)?.text()?;
                        return Ok(Some(format!("{}:{}", checksum_type, response.trim())));
                    } else {
                        return Ok(Some(format!("{}:{}", checksum_type, item.checksum.clone())));
                    }
                }
            }
            Ok(None)
        }
    }
}

fn verify_checksum(
    data: &[u8],
    method: &types::InstallationMethod,
    pkg: &types::Package,
    file_to_verify: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(checksums) = &method.checksums {
        println!("Verifying checksum for {}...", file_to_verify);
        let platform = utils::get_platform()?;
        if let Some(expected_checksum) =
            get_expected_checksum(checksums, file_to_verify, pkg, &platform)?
        {
            let (algo, expected) = if let Some((a, b)) = expected_checksum.split_once(':') {
                (a.to_lowercase(), b.to_string())
            } else {
                ("sha512".to_string(), expected_checksum)
            };
            let computed_checksum = match algo.as_str() {
                "sha256" => {
                    let mut hasher = Sha256::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
                _ => {
                    let mut hasher = Sha512::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
            };

            if computed_checksum.eq_ignore_ascii_case(&expected) {
                println!("{}", "Checksum verified successfully.".green());
                Ok(())
            } else {
                Err(format!(
                    "Checksum mismatch for {}.\nExpected: {}\nComputed: {}",
                    file_to_verify, expected, computed_checksum
                )
                .into())
            }
        } else {
            println!(
                "{} No checksum found for file '{}'. Skipping verification.",
                "Warning:".yellow(),
                file_to_verify
            );
            Ok(())
        }
    } else {
        Ok(())
    }
}

fn ensure_binary_is_cached(pkg: &types::Package) -> Result<PathBuf, Box<dyn Error>> {
    let cache_dir = cache::get_cache_root()?;
    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", pkg.name)
    } else {
        pkg.name.clone()
    };
    let bin_path = cache_dir.join(&binary_filename);

    if bin_path.exists() {
        println!("Using cached binary for '{}'.", pkg.name.cyan());
        return Ok(bin_path);
    }

    println!(
        "No cached binary found for '{}'. Downloading...",
        pkg.name.cyan()
    );
    fs::create_dir_all(&cache_dir)?;

    let platform = utils::get_platform()?;

    if let Some(method) = pkg.installation.iter().find(|m| {
        m.install_type == "binary" && utils::is_platform_compatible(&platform, &m.platforms)
    }) {
        let mut url = method
            .url
            .replace("{version}", pkg.version.as_deref().unwrap_or(""));
        url = url.replace("{name}", &pkg.name);
        url = url.replace("{platform}", &platform);

        if url.starts_with("http://") {
            println!(
                "{} downloading over insecure HTTP: {}",
                "Warning:".yellow(),
                url
            );
        }
        println!("Downloading from: {url}");

        let response = reqwest::blocking::get(&url)?;
        if !response.status().is_success() {
            return Err(format!("Failed to download binary: HTTP {}", response.status()).into());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
            .progress_chars("#>- "));

        let mut downloaded_bytes = Vec::new();
        let mut stream = response;
        let mut buffer = [0; 8192];
        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
            pb.inc(bytes_read as u64);
        }
        pb.finish_with_message("Download complete.");

        let file_to_verify = get_filename_from_url(&url);
        verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;

        let mut dest = File::create(&bin_path)?;
        dest.write_all(&downloaded_bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
        }

        return Ok(bin_path);
    }

    if let Some(method) = pkg.installation.iter().find(|m| {
        m.install_type == "com_binary" && utils::is_platform_compatible(&platform, &m.platforms)
    }) {
        let os = std::env::consts::OS;
        let com_ext = method
            .platform_com_ext
            .as_ref()
            .and_then(|ext_map| ext_map.get(os))
            .map(|s| s.as_str())
            .unwrap_or(if os == "windows" { "zip" } else { "tar.zst" });

        let mut url = method
            .url
            .replace("{version}", pkg.version.as_deref().unwrap_or(""));
        url = url.replace("{name}", &pkg.name);
        url = url.replace("{platform}", &platform);
        url = url.replace("{platformComExt}", com_ext);

        if url.starts_with("http://") {
            println!(
                "{} downloading over insecure HTTP: {}",
                "Warning:".yellow(),
                url
            );
        }
        println!("Downloading from: {url}");

        let client = crate::utils::build_blocking_http_client(60)?;
        let mut attempt = 0u32;
        let response = loop {
            attempt += 1;
            match client.get(&url).send() {
                Ok(resp) => break resp,
                Err(e) => {
                    if attempt < 3 {
                        eprintln!(
                            "{}: download failed ({}). Retrying...",
                            "Network".yellow(),
                            e
                        );
                        crate::utils::retry_backoff_sleep(attempt);
                        continue;
                    } else {
                        return Err(format!(
                            "Failed to download '{}' after {} attempts: {}",
                            url, attempt, e
                        )
                        .into());
                    }
                }
            }
        };
        if !response.status().is_success() {
            return Err(format!("Failed to download (HTTP {}): {}", response.status(), url).into());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
            .progress_chars("#>- "));

        let mut downloaded_bytes = Vec::new();
        let mut stream = response;
        let mut buffer = [0; 8192];
        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
            pb.inc(bytes_read as u64);
        }
        pb.finish_with_message("Download complete.");

        let file_to_verify = get_filename_from_url(&url);
        verify_checksum(&downloaded_bytes, method, pkg, file_to_verify)?;

        let temp_dir = Builder::new().prefix("zoi-exec-ext").tempdir()?;

        if com_ext == "zip" {
            let mut archive = ZipArchive::new(Cursor::new(downloaded_bytes))?;
            archive.extract(temp_dir.path())?;
        } else if com_ext == "tar.zst" {
            let tar = ZstdDecoder::new(Cursor::new(downloaded_bytes))?;
            let mut archive = Archive::new(tar);
            archive.unpack(temp_dir.path())?;
        } else if com_ext == "tar.xz" {
            let tar = XzDecoder::new(Cursor::new(downloaded_bytes));
            let mut archive = Archive::new(tar);
            archive.unpack(temp_dir.path())?;
        } else if com_ext == "tar.gz" {
            let tar = GzDecoder::new(Cursor::new(downloaded_bytes));
            let mut archive = Archive::new(tar);
            archive.unpack(temp_dir.path())?;
        } else {
            return Err(format!("Unsupported compression format: {}", com_ext).into());
        }

        let binary_name = &pkg.name;
        let binary_name_with_ext = format!("{}.exe", pkg.name);
        let declared_binary_path = method.binary_path.as_deref();
        let mut found_binary_path = None;
        let mut files_in_archive = Vec::new();

        for entry in WalkDir::new(temp_dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            let path = entry.path();
            files_in_archive.push(path.to_path_buf());
            if let Some(bp) = declared_binary_path {
                let rel = path
                    .strip_prefix(temp_dir.path())
                    .unwrap_or(path)
                    .to_path_buf();
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let bp_norm = bp.replace('\\', "/");
                let file_name = path.file_name().and_then(|o| o.to_str()).unwrap_or("");
                let mut matched = rel_str == bp_norm;
                if !matched && !bp_norm.contains('/') {
                    matched = file_name == bp_norm
                        || (cfg!(target_os = "windows")
                            && bp_norm == binary_name.as_str()
                            && file_name == binary_name_with_ext.as_str());
                }
                if matched {
                    found_binary_path = Some(path.to_path_buf());
                }
            } else {
                let file_name = path.file_name().unwrap_or_default();
                if file_name == binary_name.as_str()
                    || (cfg!(target_os = "windows") && file_name == binary_name_with_ext.as_str())
                {
                    found_binary_path = Some(path.to_path_buf());
                }
            }
        }

        if let Some(found_path) = found_binary_path {
            if found_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".exe"))
                .unwrap_or(false)
            {
                let cache_dir = cache::get_cache_root()?;
                let bin_path_new = cache_dir.join(format!("{}.exe", pkg.name));
                fs::copy(found_path, &bin_path_new)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&bin_path_new, fs::Permissions::from_mode(0o755))?;
                }
                return Ok(bin_path_new);
            }
            fs::copy(found_path, &bin_path)?;
        } else if files_in_archive.len() == 1 {
            println!(
                "{}",
                "Could not find binary by package name. Found one file, assuming it's the correct one."
                    .yellow()
            );
            let only = &files_in_archive[0];
            if only
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".exe"))
                .unwrap_or(false)
            {
                let cache_dir = cache::get_cache_root()?;
                let bin_path_new = cache_dir.join(format!("{}.exe", pkg.name));
                fs::copy(only, &bin_path_new)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&bin_path_new, fs::Permissions::from_mode(0o755))?;
                }
                return Ok(bin_path_new);
            }
            fs::copy(only, &bin_path)?;
        } else {
            return Err(format!(
                "Could not find binary '{}' in the extracted archive.",
                binary_name
            )
            .into());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
        }

        return Ok(bin_path);
    }

    Err("No compatible 'binary' or 'com_binary' installation method found for this package.".into())
}

fn find_executable(pkg: &types::Package) -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(Some(_)) = local::is_package_installed(&pkg.name, types::Scope::User) {
        let store_root = local::get_store_root(types::Scope::User)?;
        let binary_filename = if cfg!(target_os = "windows") {
            format!("{}.exe", pkg.name)
        } else {
            pkg.name.clone()
        };
        let path = store_root
            .join(&pkg.name)
            .join("bin")
            .join(&binary_filename);
        if path.exists() {
            println!("Using user-installed binary for '{}'.", pkg.name.cyan());
            return Ok(path);
        }
    }

    if let Ok(Some(_)) = local::is_package_installed(&pkg.name, types::Scope::System) {
        let store_root = local::get_store_root(types::Scope::System)?;
        let binary_filename = if cfg!(target_os = "windows") {
            format!("{}.exe", pkg.name)
        } else {
            pkg.name.clone()
        };
        let path = store_root
            .join(&pkg.name)
            .join("bin")
            .join(&binary_filename);
        if path.exists() {
            println!("Using system-installed binary for '{}'.", pkg.name.cyan());
            return Ok(path);
        }
    }

    ensure_binary_is_cached(pkg)
}

pub fn run(source: &str, args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let resolved_source = resolve::resolve_source(source)?;

    utils::print_repo_warning(&resolved_source.repo_name);

    let content = fs::read_to_string(&resolved_source.path)?;
    let pkg: types::Package = serde_yaml::from_str(&content)?;

    if pkg.package_type == types::PackageType::App {
        return Err("This package is an 'app' template. Use 'zoi create <pkg> <appName>' to create an app from it.".into());
    }

    let bin_path = find_executable(&pkg)?;

    match crate::pkg::telemetry::posthog_capture_event("exec", &pkg, env!("CARGO_PKG_VERSION")) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    println!("\n--- Executing '{}' ---\n", pkg.name.bold());
    let status = Command::new(bin_path).args(args).status()?;

    if let Some(code) = status.code() {
        std::process::exit(code);
    }

    Ok(())
}
