use crate::pkg::{cache, install::resolver::InstallNode, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use home;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sha2::{Digest, Sha512};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn send_telemetry(event: &str, pkg: &types::Package, registry_handle: &str) {
    match crate::pkg::telemetry::posthog_capture_event(
        event,
        pkg,
        env!("CARGO_PKG_VERSION"),
        registry_handle,
    ) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }
}

pub fn display_updates(pkg: &types::Package, yes: bool) -> Result<bool> {
    if let Some(updates) = &pkg.updates {
        if updates.is_empty() {
            return Ok(true);
        }
        println!("\n{}", "Important Updates:".bold().yellow());
        for update in updates {
            let type_str = match update.update_type {
                types::UpdateType::Change => "Change".blue(),
                types::UpdateType::Vulnerability => "Vulnerability".red().bold(),
                types::UpdateType::Update => "Update".green(),
            };
            println!("  - [{}] {}", type_str, update.message);
        }

        if !utils::ask_for_confirmation("\nDo you want to continue?", yes) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn get_conflicts(
    pkg: &types::Package,
    installed_packages: &[types::InstallManifest],
) -> Result<Vec<String>> {
    let mut conflict_messages = Vec::new();

    if let Some(conflicts_with) = &pkg.conflicts {
        for conflict_pkg_name in conflicts_with {
            let is_zoi_conflict = installed_packages
                .iter()
                .any(|p| &p.name == conflict_pkg_name);

            if is_zoi_conflict {
                conflict_messages.push(format!(
                    "Package '{}' conflicts with installed package '{}'.",
                    pkg.name, conflict_pkg_name
                ));
            } else if utils::command_exists(conflict_pkg_name) {
                conflict_messages.push(format!(
                    "Package '{}' conflicts with existing command '{}' on your system.",
                    pkg.name, conflict_pkg_name
                ));
            }
        }
    }

    if let Some(bins_provided) = &pkg.bins {
        for bin in bins_provided {
            for installed_pkg in installed_packages {
                if let Some(installed_bins) = &installed_pkg.bins
                    && installed_bins.contains(bin)
                {
                    conflict_messages.push(format!(
                            "Binary '{}' provided by '{}' is already provided by installed package '{}'.",
                            bin, pkg.name, installed_pkg.name
                        ));
                }
            }
        }
    }

    if let Some(provides) = &pkg.provides {
        for p in provides {
            for installed_pkg in installed_packages {
                if let Some(installed_provides) = &installed_pkg.provides
                    && installed_provides.contains(p)
                {
                    conflict_messages.push(format!(
                            "Virtual package '{}' provided by '{}' is already provided by installed package '{}'.",
                            p, pkg.name, installed_pkg.name
                        ));
                }
            }
        }
    }

    Ok(conflict_messages)
}

pub fn check_for_conflicts(packages_to_install: &[&types::Package], yes: bool) -> Result<()> {
    let installed_packages = crate::pkg::local::get_installed_packages()?;
    let mut all_conflict_messages = HashSet::new();

    for pkg in packages_to_install {
        let conflicts = get_conflicts(pkg, &installed_packages)?;
        all_conflict_messages.extend(conflicts);
    }

    if !all_conflict_messages.is_empty() {
        println!("\n{}", "Conflict Detected:".red().bold());
        for msg in &all_conflict_messages {
            println!("- {}", msg);
        }
        if !utils::ask_for_confirmation(
            "\nDo you want to continue with the installation anyway?",
            yes,
        ) {
            return Err(anyhow!("Operation aborted by user due to conflicts."));
        }
    }

    Ok(())
}

pub fn get_filename_from_url(url: &str) -> &str {
    url.split('/').next_back().unwrap_or_default()
}

pub fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    m: Option<&MultiProgress>,
    expected_size: Option<u64>,
) -> Result<()> {
    if url.starts_with("http://") {
        let msg = format!("downloading over insecure HTTP: {}", url);
        if m.is_none() {
            println!("{}: {}", "Warning:".yellow(), msg);
        }
    }

    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} {msg:30.cyan.bold} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {elapsed_precise})")?
        .progress_chars("=>-");

    let mut pb = if let Some(m) = m {
        let pb = m.add(ProgressBar::new(expected_size.unwrap_or(0)));
        pb.set_style(pb_style.clone());
        pb.set_message(format!("Connecting to {}", get_filename_from_url(url)));
        pb
    } else {
        println!("Downloading from: {url}");
        ProgressBar::new(expected_size.unwrap_or(0))
    };

    let client = crate::utils::build_blocking_http_client(60)?;
    let mut attempt = 0u32;

    let mut partial_size = 0;
    if dest_path.exists() {
        partial_size = dest_path.metadata()?.len();
    }

    let mut request = client.get(url);
    if partial_size > 0 {
        let msg = format!("Resuming download from byte {}", partial_size);
        if m.is_some() {
            pb.set_message(msg);
        } else {
            println!("{}", msg);
        }
        request = request.header("Range", format!("bytes={}-", partial_size));
    }

    let response = loop {
        attempt += 1;
        match request
            .try_clone()
            .ok_or_else(|| anyhow!("Failed to clone request"))?
            .send()
        {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < 3 {
                    let msg = format!("Download failed ({}). Retrying...", e);
                    if m.is_some() {
                        pb.set_message(msg);
                    } else {
                        eprintln!("{}: {}", "Network".yellow(), msg);
                    }
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(anyhow!(
                        "Failed to download '{}' after {} attempts: {}",
                        url,
                        attempt,
                        e
                    ));
                }
            }
        }
    };

    let mut is_resumed = false;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        is_resumed = true;
    } else if response.status().is_success() {
        partial_size = 0;
    } else {
        return Err(anyhow!(
            "Failed to download (HTTP {}): {}",
            response.status(),
            url
        ));
    }

    let total_size = if let Some(s) = expected_size {
        s
    } else {
        partial_size + response.content_length().unwrap_or(0)
    };

    if m.is_none() {
        let new_pb = ProgressBar::new(total_size);
        new_pb.set_style(pb_style);
        pb.finish_and_clear();
        let _ = std::mem::replace(&mut pb, new_pb);
    }

    pb.set_length(total_size);
    pb.set_position(partial_size);
    pb.set_message(format!("Downloading {}", get_filename_from_url(url)));

    let mut dest_file = if is_resumed {
        std::fs::OpenOptions::new().append(true).open(dest_path)?
    } else {
        File::create(dest_path)?
    };

    let mut stream = response;
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest_file.write_all(&buffer[..bytes_read])?;
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message(format!("Downloaded {}", get_filename_from_url(url)));
    Ok(())
}

pub fn verify_file_hash(file_path: &Path, expected_hash: &str) -> Result<bool> {
    println!("Verifying hash for {}...", file_path.display());
    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    std::io::copy(&mut file, &mut hasher)?;
    let actual_hash = hex::encode(hasher.finalize());
    let result = actual_hash.eq_ignore_ascii_case(expected_hash);
    if result {
        println!("{}", "Hash verified successfully.".green());
    } else {
        println!(
            "{}\nExpected: {}\nActual:   {}",
            "Hash mismatch!".red(),
            expected_hash,
            actual_hash
        );
    }
    Ok(result)
}

pub fn check_file_conflicts(
    graph: &super::resolver::DependencyGraph,
    yes: bool,
    _m: &MultiProgress,
) -> Result<()> {
    let mut all_conflicts = HashSet::new();

    for node in graph.nodes.values() {
        if let Ok(Some(info)) = find_prebuilt_info(node) {
            let archive_filename = info.final_url.split('/').next_back().unwrap_or_default();
            let archive_cache_root = match cache::get_archive_cache_root() {
                Ok(path) => path,
                Err(_) => continue,
            };
            let archive_path = archive_cache_root.join(archive_filename);

            if !archive_path.exists() {
                continue;
            }
            let request = match crate::pkg::resolve::parse_source_string(&node.source) {
                Ok(req) => req,
                Err(_) => {
                    eprintln!(
                        "{} Could not parse source string for {} to check for file conflicts.",
                        "Warning:".yellow(),
                        node.pkg.name
                    );
                    continue;
                }
            };

            if let Ok(conflicts) = get_file_conflicts_from_archive(
                &archive_path,
                &node.pkg,
                request.sub_package.as_deref(),
            ) {
                for conflict in conflicts {
                    all_conflicts.insert(format!(
                        "File '{}' from package '{}' already exists on filesystem.",
                        conflict, node.pkg.name
                    ));
                }
            } else {
                eprintln!(
                    "{} Could not parse archive for {} to check for file conflicts.",
                    "Warning:".yellow(),
                    node.pkg.name
                );
            }
        }
    }

    if !all_conflicts.is_empty() {
        println!("\n{}", "File Conflict Detected:".red().bold());
        for msg in &all_conflicts {
            println!("- {}", msg);
        }
        if !utils::ask_for_confirmation(
            "\nDo you want to overwrite these files and continue with the installation?",
            yes,
        ) {
            return Err(anyhow!("Operation aborted by user due to file conflicts."));
        }
    }

    Ok(())
}

pub fn get_file_conflicts_from_archive(
    archive_path: &Path,
    pkg: &types::Package,
    sub_package_to_check: Option<&str>,
) -> Result<Vec<String>> {
    let file = File::open(archive_path)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);
    let temp_dir = Builder::new().prefix("zoi-conflict-check-").tempdir()?;
    archive.unpack(temp_dir.path())?;

    let mut conflicts = Vec::new();
    let data_dir = temp_dir.path().join("data");
    if !data_dir.exists() {
        return Ok(conflicts);
    }

    let subs_to_check = if let Some(sub) = sub_package_to_check {
        vec![sub.to_string()]
    } else {
        vec!["".to_string()]
    };

    for sub in subs_to_check {
        let sub_data_dir = if sub.is_empty() {
            data_dir.clone()
        } else {
            data_dir.join(&sub)
        };

        if !sub_data_dir.exists() {
            continue;
        }

        let usrroot_src = sub_data_dir.join("usrroot");
        if usrroot_src.exists() && pkg.scope == types::Scope::System {
            let root_dest = PathBuf::from("/");
            for entry in WalkDir::new(&usrroot_src)
                .into_iter()
                .filter_map(|e| e.ok())
                .skip(1)
            {
                if entry.file_type().is_file() {
                    let relative_path = entry.path().strip_prefix(&usrroot_src)?;
                    let dest_path = root_dest.join(relative_path);
                    if dest_path.exists() {
                        conflicts.push(dest_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        let usrhome_src = sub_data_dir.join("usrhome");
        if usrhome_src.exists()
            && let Some(home_dest) = home::home_dir()
        {
            for entry in WalkDir::new(&usrhome_src)
                .into_iter()
                .filter_map(|e| e.ok())
                .skip(1)
            {
                if entry.file_type().is_file() {
                    let relative_path = entry.path().strip_prefix(&usrhome_src)?;
                    let dest_path = home_dest.join(relative_path);
                    if dest_path.exists() {
                        conflicts.push(dest_path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    Ok(conflicts)
}

pub fn get_expected_hash(hash_url: &str) -> Result<String> {
    println!("Fetching hash from: {}", hash_url);
    let client = crate::utils::build_blocking_http_client(10)?;
    let resp = client.get(hash_url).send()?.text()?;
    Ok(resp
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string())
}

pub fn get_expected_size(size_url: &str) -> Result<u64> {
    let client = crate::utils::build_blocking_http_client(10)?;
    let resp = client.get(size_url).send()?.text()?;
    let size = resp.trim().parse::<u64>()?;
    Ok(size)
}

pub fn find_prebuilt_info(node: &InstallNode) -> Result<Option<types::PrebuiltInfo>> {
    let pkg = &node.pkg;
    let platform = crate::utils::get_platform()?;

    let db_path = crate::pkg::resolve::get_db_root()?;
    let repo_db_path = db_path.join(&node.registry_handle);
    if let Ok(repo_config) = crate::pkg::config::read_repo_config(&repo_db_path) {
        let mut pkg_links_to_try = Vec::new();
        if let Some(main_pkg) = repo_config.pkg.iter().find(|p| p.link_type == "main") {
            pkg_links_to_try.push(main_pkg.clone());
        }
        pkg_links_to_try.extend(
            repo_config
                .pkg
                .iter()
                .filter(|p| p.link_type == "mirror")
                .cloned(),
        );

        if let Some(pkg_link) = pkg_links_to_try.into_iter().next() {
            let (os, arch) = (
                platform.split('-').next().unwrap_or_default(),
                platform.split('-').nth(1).unwrap_or_default(),
            );

            let replace_vars = |url: &str| {
                url.replace("{os}", os)
                    .replace("{arch}", arch)
                    .replace("{version}", &node.version)
                    .replace("{repo}", &pkg.repo)
            };

            let url_dir = replace_vars(&pkg_link.url);
            let archive_filename =
                format!("{}-{}-{}.pkg.tar.zst", pkg.name, &node.version, platform);
            let final_url = format!("{}/{}", url_dir.trim_end_matches('/'), archive_filename);

            let pgp_url = pkg_link.pgp.as_ref().map(|url| replace_vars(url));
            let hash_url = pkg_link.hash.as_ref().map(|url| replace_vars(url));
            let size_url = pkg_link.size.as_ref().map(|url| replace_vars(url));

            return Ok(Some(types::PrebuiltInfo {
                final_url,
                pgp_url,
                hash_url,
                size_url,
            }));
        }
    }

    Ok(None)
}
