use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use zoi_core::{cache, types, utils};
use zoi_db as db;

use crate::resolver::InstallNode;

/// The number of times to retry a download before giving up.
static DOWNLOAD_RETRY_ATTEMPTS: AtomicU32 = AtomicU32::new(3);

/// Sets the number of download retry attempts.
pub fn set_download_retry_attempts(attempts: u32) {
    let normalized = attempts.max(1);
    DOWNLOAD_RETRY_ATTEMPTS.store(normalized, Ordering::Relaxed);
}

/// Gets the number of download retry attempts.
fn get_download_retry_attempts() -> u32 {
    DOWNLOAD_RETRY_ATTEMPTS.load(Ordering::Relaxed).max(1)
}

/// Sends telemetry for a package installation event.
pub fn send_telemetry(
    event: &str,
    pkg: &types::Package,
    registry_handle: &str,
    install_type: Option<&str>
) {
    match zoi_telemetry::posthog_capture_event(
        event,
        pkg,
        env!("CARGO_PKG_VERSION"),
        registry_handle,
        install_type
    ) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e)
    }
}

/// Displays important updates from the package metadata to the user.
///
/// # Errors
///
/// Returns an error if the user aborts the operation after being warned about
/// updates.
pub fn display_updates(pkg: &types::Package, yes: bool) -> Result<bool> {
    if let Some(updates) = &pkg.updates {
        if updates.is_empty() {
            return Ok(true);
        }
        println!("\n{}", "Important Updates:".bold().yellow());
        for update in updates {
            let type_str = match update.update_type {
                types::UpdateType::Change => "Change".blue(),
                types::UpdateType::Vulnerability => {
                    "Vulnerability".red().bold()
                }
                types::UpdateType::Update => "Update".green()
            };
            println!("  - [{}] {}", type_str, update.message);
        }

        if !utils::ask_for_confirmation("\nDo you want to continue?", yes) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Extracts the filename from a URL.
pub fn get_filename_from_url(url: &str) -> &str {
    url.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or_default()
}

/// Fetches the text content from a list of candidate URLs.
///
/// # Errors
///
/// Returns an error if the text content cannot be fetched from any of the
/// candidate URLs.
pub fn get_text_from_candidate_urls(
    urls: &[String],
    resource_name: &str
) -> Result<String> {
    let client = zoi_core::utils::get_http_client()?;
    let mut last_error = None;

    for candidate_url in urls {
        match client.get(candidate_url).send() {
            Ok(response) => match response.text() {
                Ok(text) => return Ok(text),
                Err(e) => last_error = Some(format!("{candidate_url} ({e})"))
            },
            Err(e) => last_error = Some(format!("{candidate_url} ({e})"))
        }
    }

    Err(anyhow!(
        "Failed to fetch {} from any configured source: {}",
        resource_name,
        last_error
            .unwrap_or_else(|| "no candidate URLs were attempted".to_string())
    ))
}

/// Downloads a file from a URL with progress reporting.
///
/// # Errors
///
/// Returns an error if the download fails after all retry attempts,
/// if the destination file cannot be created, or if the progress bar fails.
pub fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    pb_override: Option<&ProgressBar>,
    expected_size: Option<u64>
) -> Result<()> {
    if url.starts_with("http://") {
        let msg = format!("downloading over insecure HTTP: {url}");
        if pb_override.is_none() {
            println!("{}: {}", "Warning:".yellow(), msg);
        }
    }

    let pb_style = ProgressStyle::default_bar()
        .template(
            "{spinner:.green} {msg:30.cyan.bold} [{bar:40.cyan/blue}] \
             {bytes}/{total_bytes} ({bytes_per_sec}, {elapsed_precise})"
        )?
        .progress_chars("=>-");

    let mut internal_pb = None;
    let pb = if let Some(p) = pb_override {
        p.set_style(pb_style.clone());
        p.set_length(expected_size.unwrap_or(0));
        p.set_message(format!("Downloading {}", get_filename_from_url(url)));
        p
    } else {
        let p = ProgressBar::new(expected_size.unwrap_or(0));
        p.set_style(pb_style);
        p.set_message(format!("Downloading {}", get_filename_from_url(url)));
        internal_pb = Some(p);
        internal_pb.as_ref().ok_or_else(|| {
            anyhow!("internal_pb should be set if not using pb_override")
        })?
    };

    let client = zoi_core::utils::get_http_client()?;
    let mut attempt = 0u32;

    let mut partial_size = 0;
    if dest_path.exists() {
        partial_size = dest_path.metadata()?.len();
    }

    let mut request = client.get(url);
    if partial_size > 0 {
        let msg = format!("Resuming download from byte {partial_size}");
        pb.set_message(msg);
        request = request.header("Range", format!("bytes={partial_size}-"));
    }

    let max_attempts = get_download_retry_attempts();
    let response = loop {
        attempt += 1;
        match request
            .try_clone()
            .ok_or_else(|| anyhow!("Failed to clone request"))?
            .send()
        {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < max_attempts {
                    let msg = format!("Download failed ({e}). Retrying...");
                    pb.set_message(msg);
                    zoi_core::utils::retry_backoff_sleep(attempt);
                    continue;
                }
                return Err(anyhow!(
                    "Failed to download '{url}' after {attempt} attempts: {e}"
                ));
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
        dest_file.write_all(
            buffer
                .get(..bytes_read)
                .ok_or_else(|| anyhow!("buffer slice out of bounds"))?
        )?;
        pb.inc(bytes_read as u64);
    }

    if let Some(p) = internal_pb {
        p.finish_and_clear();
        println!("Downloaded {}", get_filename_from_url(url));
    }
    Ok(())
}

/// Verifies the hash of a file against an expected hash.
///
/// # Errors
///
/// Returns an error if the hash algorithm is unsupported or if the hash
/// calculation fails.
pub fn verify_file_hash(
    file_path: &Path,
    expected_hash: &str,
    pb: Option<&ProgressBar>
) -> Result<bool> {
    let expected_clean = expected_hash.trim().to_lowercase();
    let Some(algo) =
        zoi_core::hash::HashAlgorithm::from_len(expected_clean.len())
    else {
        return Err(anyhow!(
            "Unsupported hash length: {}. Expected 128 (SHA-512) or 64 \
             (SHA-256).",
            expected_clean.len()
        ));
    };

    let actual_hash = zoi_core::hash::calculate_file_hash(file_path, algo)?;
    let actual_clean = actual_hash.trim().to_lowercase();

    let result = actual_clean == expected_clean;
    if result {
        let msg = format!(
            "{} Hash verified: {}",
            "::".bold().blue(),
            expected_clean[..12].dimmed()
        );
        if let Some(p) = pb {
            p.println(msg);
        } else {
            println!("{msg}");
        }
    } else {
        let mut msg = format!("{}\n", "Hash verification failed!".red().bold());
        let _ = writeln!(msg, "  Expected: {}", expected_clean.yellow());
        let _ = writeln!(msg, "  Actual:   {}", actual_clean.cyan());
        let _ = write!(
            msg,
            "  Lengths:  Expected={}, Actual={}",
            expected_clean.len(),
            actual_clean.len()
        );

        if let Some(p) = pb {
            p.println(msg);
        } else {
            println!("{msg}");
        }
    }
    Ok(result)
}

/// Fetches a list of files from a remote registry.
///
/// # Errors
///
/// Returns an error if the list of files cannot be fetched from the remote
/// registry.
pub fn get_remote_file_list(url: &str) -> Result<Vec<String>> {
    if zoi_core::offline::is_offline() {
        return Ok(Vec::new());
    }
    let resp = get_text_from_candidate_urls(
        &cache::mirror_candidate_urls(url),
        "files list"
    )?;

    Ok(resp
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Fetches the expected hash for a file from a remote source.
///
/// # Errors
///
/// Returns an error if the expected hash cannot be fetched from the remote
/// source.
pub fn get_expected_hash(
    hash_url: &str,
    filename: Option<&str>
) -> Result<String> {
    if zoi_core::offline::is_offline() {
        return Ok(String::new());
    }
    let resp = get_text_from_candidate_urls(
        &cache::mirror_candidate_urls(hash_url),
        "hash file"
    )?;

    let is_valid_hash = |s: &str| {
        let len = s.len();
        (len == 128 || len == 64 || len == 32)
            && s.chars().all(|c| c.is_ascii_hexdigit())
    };

    if let Some(target_file) = filename {
        for line in resp.lines() {
            if line.contains(target_file) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(hash) = parts.iter().find(|&&p| is_valid_hash(p)) {
                    return Ok(hash.to_string());
                }
            }
        }
    }

    for word in resp.split_whitespace() {
        if is_valid_hash(word) {
            return Ok(word.to_string());
        }
    }

    Ok(resp
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string())
}

/// Fetches the expected download and installed sizes for a package.
///
/// # Errors
///
/// Returns an error if the expected size cannot be fetched from the remote
/// source.
pub fn get_expected_size(size_url: &str) -> Result<(u64, u64)> {
    if zoi_core::offline::is_offline() {
        return Ok((0, 0));
    }
    let resp = get_text_from_candidate_urls(
        &cache::mirror_candidate_urls(size_url),
        "size file"
    )?;

    let mut download_size = 0;
    let mut installed_size = 0;
    let mut found_fields = false;

    for line in resp.lines() {
        if let Some((key, val)) = line.split_once(':')
            && let Ok(num) = val.trim().parse::<u64>()
        {
            match key.trim() {
                "down" => {
                    download_size = num;
                    found_fields = true;
                }
                "install" => {
                    installed_size = num;
                    found_fields = true;
                }
                _ => {}
            }
        }
    }

    if !found_fields && let Ok(num) = resp.trim().parse::<u64>() {
        download_size = num;
    }

    Ok((download_size, installed_size))
}

/// Resolves placeholders in a URL with actual values.
pub fn resolve_url_placeholders(
    url: &str,
    pkg_name: &str,
    repo: &str,
    version: &str,
    platform: &str
) -> String {
    let (os, arch) = (
        platform.split('-').next().unwrap_or_default(),
        platform.split('-').nth(1).unwrap_or_default()
    );

    let id = if repo.is_empty() {
        pkg_name.to_string()
    } else {
        format!("{}.{}", repo.replace('/', "."), pkg_name)
    };

    url.replace("{os}", os)
        .replace("{arch}", arch)
        .replace("{version}", version)
        .replace("{repo}", repo)
        .replace("{name}", pkg_name)
        .replace("{id}", &id)
        .replace("{platform}", platform)
}

/// Finds prebuilt info for a package in the registry.
///
/// # Errors
///
/// Returns an error if the database root or repository configuration cannot be
/// read.
pub fn find_registry_info_for_package(
    pkg: &types::Package,
    registry_handle: &str,
    version: &str,
    is_source: bool
) -> Result<Option<types::PrebuiltInfo>> {
    let platform = zoi_core::utils::get_platform()?;

    let repo_config =
        if zoi_core::utils::is_mini_mode() && registry_handle == "zoidberg" {
            zoi_resolver::mini_resolve::fetch_registry_config().ok()
        } else {
            let db_path = zoi_resolver::resolve::get_db_root()?;
            let repo_db_path = db_path.join(registry_handle);
            zoi_core::config::read_repo_config(&repo_db_path).ok()
        };

    if let Some(repo_config) = repo_config {
        let main_type = if is_source { "source-main" } else { "main" };
        let mirror_type = if is_source { "source-mirror" } else { "mirror" };
        let extension = if is_source { ".zsa" } else { ".zpa" };

        let mut pkg_links_to_try = Vec::new();
        if let Some(main_pkg) =
            repo_config.pkg.iter().find(|p| p.link_type == main_type)
        {
            pkg_links_to_try.push(main_pkg.clone());
        }
        pkg_links_to_try.extend(
            repo_config
                .pkg
                .iter()
                .filter(|p| p.link_type == mirror_type)
                .cloned()
        );

        if let Some(pkg_link) = pkg_links_to_try.into_iter().next() {
            let final_url_base = resolve_url_placeholders(
                &pkg_link.url,
                &pkg.name,
                &pkg.repo,
                version,
                &platform
            );
            let final_url = if final_url_base.ends_with(extension) {
                final_url_base
            } else {
                let archive_filename = if is_source {
                    format!("{}-{}{}", pkg.name, version, extension)
                } else {
                    format!(
                        "{}-{}-{}{}",
                        pkg.name, version, platform, extension
                    )
                };
                format!(
                    "{}/{}",
                    final_url_base.trim_end_matches('/'),
                    archive_filename
                )
            };

            let pgp_url = Some(pkg_link.pgp.as_ref().map_or_else(
                || format!("{final_url}.sig"),
                |url| {
                    resolve_url_placeholders(
                        url, &pkg.name, &pkg.repo, version, &platform
                    )
                }
            ));
            let hash_url = pkg_link.hash.as_ref().map(|url| {
                resolve_url_placeholders(
                    url, &pkg.name, &pkg.repo, version, &platform
                )
            });
            let size_url = pkg_link.size.as_ref().map(|url| {
                resolve_url_placeholders(
                    url, &pkg.name, &pkg.repo, version, &platform
                )
            });
            let files_url = pkg_link.files.as_ref().map(|url| {
                resolve_url_placeholders(
                    url, &pkg.name, &pkg.repo, version, &platform
                )
            });

            return Ok(Some(types::PrebuiltInfo {
                final_url,
                pgp_url,
                hash_url,
                size_url,
                files_url
            }));
        }
    }

    Ok(None)
}

/// Finds prebuilt info for a prebuilt package.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_prebuilt_info_for_package(
    pkg: &types::Package,
    registry_handle: &str,
    version: &str
) -> Result<Option<types::PrebuiltInfo>> {
    find_registry_info_for_package(pkg, registry_handle, version, false)
}

/// Finds info for a source bundle package.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_source_bundle_info_for_package(
    pkg: &types::Package,
    registry_handle: &str,
    version: &str
) -> Result<Option<types::PrebuiltInfo>> {
    find_registry_info_for_package(pkg, registry_handle, version, true)
}

/// Finds delta patch info for upgrading from one version to another.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_delta_info(
    pkg: &types::Package,
    registry_handle: &str,
    _from_version: &str,
    to_version: &str
) -> Result<Option<types::DeltaInfo>> {
    let platform = zoi_core::utils::get_platform()?;

    let repo_config =
        if zoi_core::utils::is_mini_mode() && registry_handle == "zoidberg" {
            zoi_resolver::mini_resolve::fetch_registry_config().ok()
        } else {
            let db_path = zoi_resolver::resolve::get_db_root()?;
            let repo_db_path = db_path.join(registry_handle);
            zoi_core::config::read_repo_config(&repo_db_path).ok()
        };

    if let Some(repo_config) = repo_config {
        let mut delta_links_to_try = Vec::new();
        // Try "main" type first, then "mirror"
        if let Some(main_delta) =
            repo_config.delta.iter().find(|d| d.link_type == "main")
        {
            delta_links_to_try.push(main_delta.clone());
        }
        delta_links_to_try.extend(
            repo_config
                .delta
                .iter()
                .filter(|d| d.link_type == "mirror")
                .cloned()
        );

        if let Some(delta_link) = delta_links_to_try.into_iter().next() {
            // Delta URLs are expected to contain {name}, {version} placeholders
            // The version in the URL refers to the "to" version (target
            // version)
            let final_url = resolve_url_placeholders(
                &delta_link.url,
                &pkg.name,
                &pkg.repo,
                to_version,
                &platform
            );

            let pgp_url = Some(delta_link.pgp.as_ref().map_or_else(
                || format!("{final_url}.sig"),
                |url| {
                    resolve_url_placeholders(
                        url, &pkg.name, &pkg.repo, to_version, &platform
                    )
                }
            ));
            let hash_url = delta_link.hash.as_ref().map(|url| {
                resolve_url_placeholders(
                    url, &pkg.name, &pkg.repo, to_version, &platform
                )
            });
            let size_url = delta_link.size.as_ref().map(|url| {
                resolve_url_placeholders(
                    url, &pkg.name, &pkg.repo, to_version, &platform
                )
            });

            return Ok(Some(types::DeltaInfo {
                final_url,
                pgp_url,
                hash_url,
                size_url
            }));
        }
    }

    Ok(None)
}

/// Finds delta patch info for an install node, given the base version.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_delta_info_for_node(
    node: &InstallNode,
    base_version: &str
) -> Result<Option<types::DeltaInfo>> {
    find_delta_info(
        &node.pkg,
        &node.registry_handle,
        base_version,
        &node.version
    )
}

/// Finds prebuilt info for an install node.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_prebuilt_info(
    node: &InstallNode
) -> Result<Option<types::PrebuiltInfo>> {
    find_prebuilt_info_for_package(
        &node.pkg,
        &node.registry_handle,
        &node.version
    )
}

/// Finds source bundle info for an install node.
///
/// # Errors
///
/// Returns an error if the registry information cannot be retrieved.
pub fn find_source_bundle_info(
    node: &InstallNode
) -> Result<Option<types::PrebuiltInfo>> {
    find_source_bundle_info_for_package(
        &node.pkg,
        &node.registry_handle,
        &node.version
    )
}

/// Gets the expected download and installed sizes for a package.
pub fn get_package_sizes(
    pkg: &types::Package,
    registry_handle: &str,
    version: &str
) -> (u64, u64) {
    let download_size = pkg.archive_size.unwrap_or(0);
    let installed_size = pkg.installed_size.unwrap_or(0);

    if download_size > 0 && installed_size > 0 {
        return (download_size, installed_size);
    }

    if let Ok(Some((db_down, db_inst))) = db::get_package_sizes_from_db(
        registry_handle,
        &pkg.name,
        pkg.sub_package.as_deref()
    ) {
        return (db_down, db_inst);
    }

    match find_prebuilt_info_for_package(pkg, registry_handle, version) {
        Ok(Some(info)) => {
            if let Some(size_url) = &info.size_url {
                if zoi_core::offline::is_offline() {
                    (download_size, installed_size)
                } else {
                    get_expected_size(size_url).unwrap_or_else(|e| {
                        eprintln!(
                            "Warning: could not fetch size for {}: {}. \
                             Falling back to metadata.",
                            pkg.name, e
                        );
                        (download_size, installed_size)
                    })
                }
            } else {
                (download_size, installed_size)
            }
        }
        _ => (download_size, installed_size)
    }
}

#[cfg(test)]
mod tests {
    use super::{get_download_retry_attempts, set_download_retry_attempts};

    #[test]
    fn download_retry_attempts_are_clamped_to_minimum_one() {
        let previous = get_download_retry_attempts();
        set_download_retry_attempts(0);
        assert_eq!(get_download_retry_attempts(), 1);
        set_download_retry_attempts(previous);
    }

    #[test]
    fn download_retry_attempts_accept_positive_values() {
        let previous = get_download_retry_attempts();
        set_download_retry_attempts(7);
        assert_eq!(get_download_retry_attempts(), 7);
        set_download_retry_attempts(previous);
    }
}
