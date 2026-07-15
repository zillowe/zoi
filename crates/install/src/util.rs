use crate::resolver::InstallNode;
use anyhow::{Result, anyhow};
use colored::*;
use home;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use semver::{Version, VersionReq};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use tar::Archive;
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::cache;
use zoi_core::types;
use zoi_core::utils;
use zoi_db as db;
use zstd::stream::read::Decoder as ZstdDecoder;

static DOWNLOAD_RETRY_ATTEMPTS: AtomicU32 = AtomicU32::new(3);

pub fn set_download_retry_attempts(attempts: u32) {
    let normalized = attempts.max(1);
    DOWNLOAD_RETRY_ATTEMPTS.store(normalized, Ordering::Relaxed);
}

fn get_download_retry_attempts() -> u32 {
    DOWNLOAD_RETRY_ATTEMPTS.load(Ordering::Relaxed).max(1)
}

pub fn send_telemetry(
    event: &str,
    pkg: &types::Package,
    registry_handle: &str,
    install_type: Option<&str>,
) {
    match zoi_telemetry::posthog_capture_event(
        event,
        pkg,
        env!("CARGO_PKG_VERSION"),
        registry_handle,
        install_type,
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

/// Checks for logical conflicts between a package and the current system state.
///
/// Conflict Categories:
/// - Explicit Conflicts: Packages listed in the `conflicts` metadata field.
/// - Binary Collisions: Binary names (`bins`) already provided by other packages.
/// - Virtual Collisions: Virtual packages (`provides`) already provided.
pub fn get_conflicts(
    pkg: &types::Package,
    installed_packages: &[types::InstallManifest],
) -> Result<Vec<String>> {
    let mut conflict_messages = Vec::new();

    if let Some(conflicts_with) = &pkg.conflicts {
        for conflict_pkg_name in conflicts_with {
            let is_zoi_conflict = installed_packages.iter().any(|p| {
                &p.name == conflict_pkg_name
                    && (p.name != pkg.name || p.sub_package != pkg.sub_package)
            });

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
                if installed_pkg.name == pkg.name && installed_pkg.sub_package == pkg.sub_package {
                    continue;
                }
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
                if installed_pkg.name == pkg.name && installed_pkg.sub_package == pkg.sub_package {
                    continue;
                }
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

/// Enforces that a package is being installed into an authorized scope.
///
/// If a package definition includes a `scopes` list, Zoi will block
/// installation if the target scope is not present in that list.
pub fn check_scope_compliance(graph: &super::resolver::DependencyGraph) -> Result<()> {
    for node in graph.nodes.values() {
        if let Some(allowed_scopes) = &node.pkg.scopes
            && !allowed_scopes.contains(&node.pkg.scope)
        {
            return Err(anyhow!(
                "Package '{}' is not allowed to be installed in scope {:?}. Allowed scopes: {:?}",
                node.pkg.name,
                node.pkg.scope,
                allowed_scopes
            ));
        }
    }
    Ok(())
}

pub fn check_for_conflicts(packages_to_install: &[&types::Package], yes: bool) -> Result<()> {
    let installed_packages = zoi_resolver::local::get_installed_packages()?;
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

fn package_display(node: &InstallNode) -> String {
    if let Some(sub) = &node.sub_package {
        format!("{}:{}", node.pkg.name, sub)
    } else {
        node.pkg.name.clone()
    }
}

fn package_match_candidates(node: &InstallNode) -> Vec<String> {
    let mut values = Vec::new();
    let name = node.pkg.name.to_ascii_lowercase();
    values.push(name.clone());

    if let Some(sub) = &node.sub_package {
        let sub = sub.to_ascii_lowercase();
        values.push(format!("{}:{}", name, sub));
    }

    if !node.pkg.repo.is_empty() {
        let repo = node.pkg.repo.to_ascii_lowercase();
        values.push(format!("@{}/{}", repo, name));
        if let Some(sub) = &node.sub_package {
            values.push(format!("@{}/{}:{}", repo, name, sub.to_ascii_lowercase()));
        }
        values.push(format!("#{}@{}/{}", node.registry_handle, repo, name));
        if let Some(sub) = &node.sub_package {
            values.push(format!(
                "#{}@{}/{}:{}",
                node.registry_handle,
                repo,
                name,
                sub.to_ascii_lowercase()
            ));
        }
    }

    values
}

fn rule_matches_package(rule: &str, node: &InstallNode) -> bool {
    let normalized_rule = rule.trim().to_ascii_lowercase();
    if normalized_rule.is_empty() {
        return false;
    }
    package_match_candidates(node)
        .iter()
        .any(|candidate| candidate == &normalized_rule)
}

fn rule_matches_repo(rule: &str, repo: &str) -> bool {
    let normalized_rule = rule.trim().to_ascii_lowercase();
    if normalized_rule.is_empty() {
        return false;
    }
    let normalized_repo = repo.to_ascii_lowercase();

    if normalized_rule.contains('/') {
        normalized_repo == normalized_rule
    } else {
        normalized_repo
            .split('/')
            .any(|segment| segment == normalized_rule)
    }
}

fn license_tokens(license: &str) -> HashSet<String> {
    license
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+'))
        .filter_map(|raw| {
            let token = raw.trim().to_ascii_lowercase();
            if token.is_empty() {
                return None;
            }
            if matches!(token.as_str(), "and" | "or" | "with") {
                return None;
            }
            Some(token)
        })
        .collect()
}

fn license_contains_denied(license: &str, denied: &HashSet<String>) -> bool {
    if denied.is_empty() || license.trim().is_empty() {
        return false;
    }
    let tokens = license_tokens(license);
    tokens.iter().any(|token| denied.contains(token))
}

fn license_matches_allowed(license: &str, allowed: &HashSet<String>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    if license.trim().is_empty() {
        return false;
    }

    if let Ok(expr) = spdx::Expression::parse(license) {
        return expr.evaluate(|req| match req.license {
            spdx::LicenseItem::Spdx { id, .. } => allowed.contains(&id.name.to_ascii_lowercase()),
            spdx::LicenseItem::Other { .. } => false,
        });
    }

    let tokens = license_tokens(license);
    !tokens.is_empty() && tokens.iter().any(|token| allowed.contains(token))
}

pub fn check_policy_compliance_with_policy(
    graph: &super::resolver::DependencyGraph,
    policy: &types::Policy,
) -> Result<()> {
    let allowed_packages = policy.allowed_packages.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>()
    });
    let denied_packages = policy.denied_packages.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>()
    });
    let allowed_repos = policy.allowed_repos.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>()
    });
    let denied_repos = policy.denied_repos.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<Vec<_>>()
    });
    let allowed_licenses = policy.allowed_licenses.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<HashSet<_>>()
    });
    let denied_licenses = policy.denied_licenses.as_ref().map(|rules| {
        rules
            .iter()
            .map(|r| r.trim().to_ascii_lowercase())
            .filter(|r| !r.is_empty())
            .collect::<HashSet<_>>()
    });

    let mut violations = Vec::new();

    for node in graph.nodes.values() {
        let pkg_display = package_display(node);

        if let Some(rules) = &denied_packages
            && rules.iter().any(|rule| rule_matches_package(rule, node))
        {
            violations.push(format!("{} blocked by denied package policy.", pkg_display));
        }

        if let Some(rules) = &allowed_packages
            && !rules.is_empty()
            && !rules.iter().any(|rule| rule_matches_package(rule, node))
        {
            violations.push(format!("{} is not in allowed package policy.", pkg_display));
        }

        if let Some(rules) = &denied_repos
            && rules
                .iter()
                .any(|rule| rule_matches_repo(rule, &node.pkg.repo))
        {
            violations.push(format!(
                "{} blocked by denied repository policy ('{}').",
                pkg_display, node.pkg.repo
            ));
        }

        if let Some(rules) = &allowed_repos
            && !rules.is_empty()
            && !rules
                .iter()
                .any(|rule| rule_matches_repo(rule, &node.pkg.repo))
        {
            violations.push(format!(
                "{} repository '{}' is not allowed by policy.",
                pkg_display, node.pkg.repo
            ));
        }

        if let Some(rules) = &denied_licenses
            && license_contains_denied(&node.pkg.license, rules)
        {
            violations.push(format!(
                "{} blocked by denied license policy ('{}').",
                pkg_display, node.pkg.license
            ));
        }

        if let Some(rules) = &allowed_licenses
            && !license_matches_allowed(&node.pkg.license, rules)
        {
            violations.push(format!(
                "{} license '{}' is not allowed by policy.",
                pkg_display, node.pkg.license
            ));
        }
    }

    if !violations.is_empty() {
        println!("\n{}", "POLICY VIOLATION".red().bold());
        for message in &violations {
            println!("- {}", message);
        }
        return Err(anyhow!(
            "Installation blocked by security/compliance policy."
        ));
    }

    Ok(())
}

pub fn check_policy_compliance(graph: &super::resolver::DependencyGraph) -> Result<()> {
    let config = zoi_core::config::read_config()?;
    check_policy_compliance_with_policy(graph, &config.policy)
}

pub fn check_for_vulnerabilities(
    graph: &super::resolver::DependencyGraph,
    yes: bool,
) -> Result<()> {
    let mut all_vulnerabilities = Vec::new();

    for node in graph.nodes.values() {
        if let Ok(advisories) = zoi_db::get_advisories_for_package(
            &node.registry_handle,
            &node.pkg.name,
            node.sub_package.as_deref(),
        ) {
            for adv in advisories {
                if let Ok(version) = Version::parse(&node.version)
                    && let Ok(req) = VersionReq::parse(&adv.affected_range)
                    && req.matches(&version)
                {
                    all_vulnerabilities.push((
                        adv,
                        node.version.clone(),
                        node.pkg.name.clone(),
                        node.sub_package.clone(),
                    ));
                }
            }
        }
    }

    if !all_vulnerabilities.is_empty() {
        println!("\n{}", "SECURITY WARNING".red().bold());
        for (adv, version, pkg_name, sub_pkg) in &all_vulnerabilities {
            let display_name = if let Some(sub) = sub_pkg {
                format!("{}:{}", pkg_name, sub)
            } else {
                pkg_name.clone()
            };

            println!(
                "Package {} v{} is known to be vulnerable:",
                display_name.cyan().bold(),
                version.red()
            );
            println!(
                "[{}] {} (Severity: {})",
                adv.id.dimmed(),
                adv.summary,
                match adv.severity {
                    types::Severity::Low => "Low".blue(),
                    types::Severity::Medium => "Medium".yellow(),
                    types::Severity::High => "High".red(),
                    types::Severity::Critical => "Critical".magenta().bold(),
                }
            );
            if let Some(fixed) = &adv.fixed_in {
                println!("Fixed in version: {}", fixed.green());
            }
            println!();
        }

        let config = zoi_core::config::read_config()?;
        if config.policy.advisory_enforcement_unoverridable {
            return Err(anyhow!(
                "Installation blocked by system policy due to security vulnerabilities."
            ));
        }

        if !utils::ask_for_confirmation(
            "Do you want to continue with the installation anyway?",
            yes,
        ) {
            return Err(anyhow!(
                "Operation aborted by user due to security vulnerabilities."
            ));
        }
    }

    Ok(())
}

pub fn get_filename_from_url(url: &str) -> &str {
    url.split('/').next_back().unwrap_or_default()
}

fn get_text_from_candidate_urls(urls: &[String], resource_name: &str) -> Result<String> {
    let client = zoi_core::utils::get_http_client()?;
    let mut last_error = None;

    for candidate_url in urls {
        match client.get(candidate_url).send() {
            Ok(response) => match response.text() {
                Ok(text) => return Ok(text),
                Err(e) => last_error = Some(format!("{} ({})", candidate_url, e)),
            },
            Err(e) => last_error = Some(format!("{} ({})", candidate_url, e)),
        }
    }

    Err(anyhow!(
        "Failed to fetch {} from any configured source: {}",
        resource_name,
        last_error.unwrap_or_else(|| "no candidate URLs were attempted".to_string())
    ))
}

pub fn download_file_with_progress(
    url: &str,
    dest_path: &Path,
    pb_override: Option<&ProgressBar>,
    expected_size: Option<u64>,
) -> Result<()> {
    if url.starts_with("http://") {
        let msg = format!("downloading over insecure HTTP: {}", url);
        if pb_override.is_none() {
            println!("{}: {}", "Warning:".yellow(), msg);
        }
    }

    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} {msg:30.cyan.bold} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {elapsed_precise})")?
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
        internal_pb
            .as_ref()
            .ok_or_else(|| anyhow!("internal_pb should be set if not using pb_override"))?
    };

    let client = zoi_core::utils::get_http_client()?;
    let mut attempt = 0u32;

    let mut partial_size = 0;
    if dest_path.exists() {
        partial_size = dest_path.metadata()?.len();
    }

    let mut request = client.get(url);
    if partial_size > 0 {
        let msg = format!("Resuming download from byte {}", partial_size);
        pb.set_message(msg);
        request = request.header("Range", format!("bytes={}-", partial_size));
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
                    let msg = format!("Download failed ({}). Retrying...", e);
                    pb.set_message(msg);
                    zoi_core::utils::retry_backoff_sleep(attempt);
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

    if let Some(p) = internal_pb {
        p.finish_and_clear();
        println!("Downloaded {}", get_filename_from_url(url));
    }
    Ok(())
}

pub fn verify_file_hash(
    file_path: &Path,
    expected_hash: &str,
    pb: Option<&ProgressBar>,
) -> Result<bool> {
    let expected_clean = expected_hash.trim().to_lowercase();
    let algo = match zoi_core::hash::HashAlgorithm::from_len(expected_clean.len()) {
        Some(a) => a,
        None => {
            return Err(anyhow!(
                "Unsupported hash length: {}. Expected 128 (SHA-512), 64 (SHA-256), or 32 (MD5).",
                expected_clean.len()
            ));
        }
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
            println!("{}", msg);
        }
    } else {
        let mut msg = format!("{}\n", "Hash verification failed!".red().bold());
        msg.push_str(&format!("  Expected: {}\n", expected_clean.yellow()));
        msg.push_str(&format!("  Actual:   {}\n", actual_clean.cyan()));
        msg.push_str(&format!(
            "  Lengths:  Expected={}, Actual={}",
            expected_clean.len(),
            actual_clean.len()
        ));

        if let Some(p) = pb {
            p.println(msg);
        } else {
            println!("{}", msg);
        }
    }
    Ok(result)
}

pub fn get_remote_file_list(url: &str) -> Result<Vec<String>> {
    if zoi_core::offline::is_offline() {
        return Ok(Vec::new());
    }
    let resp = get_text_from_candidate_urls(&cache::mirror_candidate_urls(url), "files list")?;

    Ok(resp
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Performs pre-emptive conflict detection against the filesystem.
///
/// Pre-flight Safety Check:
/// Instead of waiting for a conflict to occur during extraction, Zoi can:
/// - Download `.files` metadata from the registry index.
/// - Check every intended destination path against the real system.
/// - Ignore files already owned by the same package (enabling safe upgrades).
///
/// This saves bandwidth and prevents partial installation failures.
pub fn check_file_conflicts(
    graph: &super::resolver::DependencyGraph,
    yes: bool,
    m: &MultiProgress,
) -> Result<()> {
    let installed_packages = zoi_resolver::local::get_installed_packages()?;
    let all_conflicts = Mutex::new(HashSet::new());

    let nodes: Vec<&InstallNode> = graph.nodes.values().collect();
    nodes.par_iter().try_for_each(|node| {
        let sub_package_to_check = node.sub_package.as_deref();

        let owned_files: HashSet<String> = installed_packages
            .iter()
            .find(|p| p.name == node.pkg.name && p.sub_package.as_deref() == sub_package_to_check)
            .map(|p| p.installed_files.iter().cloned().collect())
            .unwrap_or_default();

        let mut conflicts_for_this_pkg = Vec::new();

        if let Ok(Some(info)) = find_prebuilt_info(node) {
            let file_list = db::get_package_files_from_db(
                &node.registry_handle,
                &node.pkg.name,
                node.sub_package.as_deref(),
                &node.pkg.repo,
            )
            .unwrap_or(None)
            .or_else(|| {
                info.files_url
                    .as_ref()
                    .and_then(|files_url| get_remote_file_list(files_url).ok())
            });

            if let Some(list) = file_list {
                if let Ok(conflicts) =
                    get_conflicts_from_list(list, &node.pkg, sub_package_to_check)
                {
                    conflicts_for_this_pkg.extend(conflicts);
                }
            } else {
                let archive_filename = info.final_url.split('/').next_back().unwrap_or_default();
                let archive_cache_root = match cache::get_archive_cache_root() {
                    Ok(path) => path,
                    Err(_) => return Ok(()),
                };
                let archive_path = archive_cache_root.join(archive_filename);

                if archive_path.exists()
                    && let Ok(conflicts) = get_file_conflicts_from_archive(
                        &archive_path,
                        &node.pkg,
                        sub_package_to_check,
                    )
                {
                    conflicts_for_this_pkg.extend(conflicts);
                }
            }
        }

        for conflict in conflicts_for_this_pkg {
            if !owned_files.contains(&conflict) {
                all_conflicts.lock().unwrap().insert(format!(
                    "File '{}' from package '{}' already exists on filesystem.",
                    conflict, node.pkg.name
                ));
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    let conflicts = all_conflicts.into_inner().unwrap();
    if !conflicts.is_empty() {
        m.println(format!("\n{}", "File Conflict Detected:".red().bold()))?;
        for msg in &conflicts {
            m.println(format!("- {}", msg))?;
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

pub fn get_conflicts_from_list(
    list: Vec<String>,
    pkg: &types::Package,
    sub_package_to_check: Option<&str>,
) -> Result<Vec<String>> {
    let mut conflicts = Vec::new();
    let sub_prefix = if let Some(sub) = sub_package_to_check {
        format!("data/{}/", sub)
    } else {
        "data/".to_string()
    };

    for path_in_archive in list {
        if !path_in_archive.starts_with(&sub_prefix) {
            continue;
        }

        let rel_to_data = &path_in_archive[sub_prefix.len()..];
        let dest_path = if let Some(stripped) = rel_to_data.strip_prefix("usrroot/") {
            if pkg.scope != types::Scope::System {
                continue;
            }
            Some(zoi_core::sysroot::apply_sysroot(
                PathBuf::from("/").join(stripped),
            ))
        } else if let Some(stripped) = rel_to_data.strip_prefix("usrhome/") {
            home::home_dir().map(|h| h.join(stripped))
        } else {
            None
        };

        if let Some(p) = dest_path
            && p.exists()
            && p.is_file()
        {
            conflicts.push(p.to_string_lossy().to_string());
        }
    }

    Ok(conflicts)
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
            let root_dest = zoi_core::sysroot::apply_sysroot(PathBuf::from("/"));
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

pub fn get_expected_hash(hash_url: &str, filename: Option<&str>) -> Result<String> {
    if zoi_core::offline::is_offline() {
        return Ok(String::new());
    }
    let resp = get_text_from_candidate_urls(&cache::mirror_candidate_urls(hash_url), "hash file")?;

    let is_valid_hash = |s: &str| {
        let len = s.len();
        (len == 128 || len == 64 || len == 32) && s.chars().all(|c| c.is_ascii_hexdigit())
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

pub fn get_expected_size(size_url: &str) -> Result<(u64, u64)> {
    if zoi_core::offline::is_offline() {
        return Ok((0, 0));
    }
    let resp = get_text_from_candidate_urls(&cache::mirror_candidate_urls(size_url), "size file")?;

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

pub fn resolve_url_placeholders(
    url: &str,
    pkg_name: &str,
    repo: &str,
    version: &str,
    platform: &str,
) -> String {
    let (os, arch) = (
        platform.split('-').next().unwrap_or_default(),
        platform.split('-').nth(1).unwrap_or_default(),
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

pub fn find_prebuilt_info_for_package(
    pkg: &types::Package,
    registry_handle: &str,
    version: &str,
) -> Result<Option<types::PrebuiltInfo>> {
    let platform = zoi_core::utils::get_platform()?;

    let repo_config = if zoi_core::utils::is_mini_mode() && registry_handle == "zoidberg" {
        zoi_resolver::mini_resolve::fetch_registry_config().ok()
    } else {
        let db_path = zoi_resolver::resolve::get_db_root()?;
        let repo_db_path = db_path.join(registry_handle);
        zoi_core::config::read_repo_config(&repo_db_path).ok()
    };

    if let Some(repo_config) = repo_config {
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
            let final_url_base =
                resolve_url_placeholders(&pkg_link.url, &pkg.name, &pkg.repo, version, &platform);
            let final_url = if final_url_base.ends_with(".pkg.tar.zst") {
                final_url_base
            } else {
                let archive_filename = format!("{}-{}-{}.pkg.tar.zst", pkg.name, version, platform);
                format!(
                    "{}/{}",
                    final_url_base.trim_end_matches('/'),
                    archive_filename
                )
            };

            let pgp_url = Some(
                pkg_link
                    .pgp
                    .as_ref()
                    .map(|url| {
                        resolve_url_placeholders(url, &pkg.name, &pkg.repo, version, &platform)
                    })
                    .unwrap_or_else(|| format!("{final_url}.sig")),
            );
            let hash_url = pkg_link
                .hash
                .as_ref()
                .map(|url| resolve_url_placeholders(url, &pkg.name, &pkg.repo, version, &platform));
            let size_url = pkg_link
                .size
                .as_ref()
                .map(|url| resolve_url_placeholders(url, &pkg.name, &pkg.repo, version, &platform));
            let files_url = pkg_link
                .files
                .as_ref()
                .map(|url| resolve_url_placeholders(url, &pkg.name, &pkg.repo, version, &platform));

            return Ok(Some(types::PrebuiltInfo {
                final_url,
                pgp_url,
                hash_url,
                size_url,
                files_url,
            }));
        }
    }

    Ok(None)
}

pub fn find_prebuilt_info(node: &InstallNode) -> Result<Option<types::PrebuiltInfo>> {
    find_prebuilt_info_for_package(&node.pkg, &node.registry_handle, &node.version)
}

pub fn get_package_sizes(pkg: &types::Package, registry_handle: &str, version: &str) -> (u64, u64) {
    let download_size = pkg.archive_size.unwrap_or(0);
    let installed_size = pkg.installed_size.unwrap_or(0);

    if download_size > 0 && installed_size > 0 {
        return (download_size, installed_size);
    }

    if let Ok(Some((db_down, db_inst))) =
        db::get_package_sizes_from_db(registry_handle, &pkg.name, pkg.sub_package.as_deref())
    {
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
                            "Warning: could not fetch size for {}: {}. Falling back to metadata.",
                            pkg.name, e
                        );
                        (download_size, installed_size)
                    })
                }
            } else {
                (download_size, installed_size)
            }
        }
        _ => (download_size, installed_size),
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
