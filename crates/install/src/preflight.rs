use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use rayon::prelude::*;
use semver::{Version, VersionReq};
use zoi_core::{types, utils};
use zoi_db as db;

use crate::resolver::InstallNode;

/// Checks for logical conflicts between a package and the current system state.
///
/// # Errors
///
/// Returns an error if installed packages cannot be retrieved.
pub fn get_conflicts(
    pkg: &types::Package,
    installed_packages: &[types::InstallManifest]
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
                    "Package '{}' conflicts with existing command '{}' on \
                     your system.",
                    pkg.name, conflict_pkg_name
                ));
            }
        }
    }

    if let Some(bins_provided) = &pkg.bins {
        for bin in bins_provided {
            for installed_pkg in installed_packages {
                if installed_pkg.name == pkg.name
                    && installed_pkg.sub_package == pkg.sub_package
                {
                    continue;
                }
                if let Some(installed_bins) = &installed_pkg.bins
                    && installed_bins.contains(bin)
                {
                    conflict_messages.push(format!(
                        "Binary '{}' provided by '{}' is already provided by \
                         installed package '{}'.",
                        bin, pkg.name, installed_pkg.name
                    ));
                }
            }
        }
    }

    if let Some(provides) = &pkg.provides {
        for p in provides {
            for installed_pkg in installed_packages {
                if installed_pkg.name == pkg.name
                    && installed_pkg.sub_package == pkg.sub_package
                {
                    continue;
                }
                if let Some(installed_provides) = &installed_pkg.provides
                    && installed_provides.contains(p)
                {
                    conflict_messages.push(format!(
                        "Virtual package '{}' provided by '{}' is already \
                         provided by installed package '{}'.",
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
/// # Errors
///
/// Returns an error if any package is being installed in a forbidden scope.
pub fn check_scope_compliance(
    graph: &crate::resolver::DependencyGraph
) -> Result<()> {
    for node in graph.nodes.values() {
        if let Some(allowed_scopes) = &node.pkg.scopes
            && !allowed_scopes.contains(&node.pkg.scope)
        {
            return Err(anyhow!(
                "Package '{}' is not allowed to be installed in scope {:?}. \
                 Allowed scopes: {:?}",
                node.pkg.name,
                node.pkg.scope,
                allowed_scopes
            ));
        }
    }
    Ok(())
}

/// Enforces `ZoiOS`-only or non-`ZoiOS`-only package constraints.
///
/// # Errors
///
/// Returns an error if any package's `zoios` constraint is violated.
pub fn check_zoios_compliance(
    graph: &crate::resolver::DependencyGraph
) -> Result<()> {
    let is_currently_zoios = zoi_core::utils::is_zoios();

    for node in graph.nodes.values() {
        if let Some(required_zoios) = node.pkg.zoios {
            if required_zoios && !is_currently_zoios {
                return Err(anyhow!(
                    "Package '{}' can only be installed on ZoiOS systems.",
                    node.pkg.name
                ));
            } else if !required_zoios && is_currently_zoios {
                return Err(anyhow!(
                    "Package '{}' cannot be installed on ZoiOS systems.",
                    node.pkg.name
                ));
            }
        }
    }
    Ok(())
}

/// Checks for conflicts between packages to be installed and existing ones.
///
/// # Errors
///
/// Returns an error if the user aborts the operation after being warned about
/// conflicts, or if installed packages cannot be retrieved.
pub fn check_for_conflicts(
    packages_to_install: &[&types::Package],
    yes: bool
) -> Result<()> {
    let installed_packages = zoi_resolver::local::get_installed_packages()?;
    let mut all_conflict_messages = HashSet::new();

    for pkg in packages_to_install {
        let conflicts = get_conflicts(pkg, &installed_packages)?;
        all_conflict_messages.extend(conflicts);
    }

    if !all_conflict_messages.is_empty() {
        println!("\n{}", "Conflict Detected:".red().bold());
        for msg in &all_conflict_messages {
            println!("- {msg}");
        }
        if !utils::ask_for_confirmation(
            "\nDo you want to continue with the installation anyway?",
            yes
        ) {
            return Err(anyhow!("Operation aborted by user due to conflicts."));
        }
    }

    Ok(())
}

/// Checks if the dependency graph complies with the system-wide policy.
///
/// # Errors
///
/// Returns an error if the graph violates the system-wide policy.
pub fn check_policy_compliance(
    graph: &crate::resolver::DependencyGraph
) -> Result<()> {
    let config = zoi_core::config::read_config()?;
    check_policy_compliance_with_policy(graph, &config.policy)
}

/// Checks if the dependency graph complies with a specific policy.
///
/// # Errors
///
/// Returns an error if any package in the graph violates the specified policy.
pub fn check_policy_compliance_with_policy(
    graph: &crate::resolver::DependencyGraph,
    policy: &types::Policy
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
        let pkg_display = if let Some(sub) = &node.sub_package {
            format!("{}:{}", node.pkg.name, sub)
        } else {
            node.pkg.name.clone()
        };

        if let Some(rules) = &denied_packages
            && rules.iter().any(|rule| rule_matches_package(rule, node))
        {
            violations.push(format!(
                "{pkg_display} blocked by denied package policy."
            ));
        }

        if let Some(rules) = &allowed_packages
            && !rules.is_empty()
            && !rules.iter().any(|rule| rule_matches_package(rule, node))
        {
            violations.push(format!(
                "{pkg_display} is not in allowed package policy."
            ));
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
            println!("- {message}");
        }
        return Err(anyhow!(
            "Installation blocked by security/compliance policy."
        ));
    }

    Ok(())
}

/// Checks if a policy rule matches a specific package.
fn rule_matches_package(rule: &str, node: &InstallNode) -> bool {
    let normalized_rule = rule.trim().to_ascii_lowercase();
    if normalized_rule.is_empty() {
        return false;
    }
    package_match_candidates(node)
        .iter()
        .any(|candidate| candidate == &normalized_rule)
}

/// Generates candidate strings for matching a package against policy rules.
fn package_match_candidates(node: &InstallNode) -> Vec<String> {
    let mut values = Vec::new();
    let name = node.pkg.name.to_ascii_lowercase();
    values.push(name.clone());

    if let Some(sub) = &node.sub_package {
        let sub = sub.to_ascii_lowercase();
        values.push(format!("{name}:{sub}"));
    }

    if !node.pkg.repo.is_empty() {
        let repo = node.pkg.repo.to_ascii_lowercase();
        values.push(format!("@{repo}/{name}"));
        if let Some(sub) = &node.sub_package {
            values.push(format!(
                "@{}/{}:{}",
                repo,
                name,
                sub.to_ascii_lowercase()
            ));
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

/// Checks if a policy rule matches a repository.
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

/// Checks if a license string contains any denied license identifiers.
fn license_contains_denied(license: &str, denied: &HashSet<String>) -> bool {
    if denied.is_empty() || license.trim().is_empty() {
        return false;
    }

    if let Ok(expr) = spdx::Expression::parse(license) {
        // An expression counts as "denied" only when it cannot be satisfied
        // without relying on a denied license. For `A OR B` where only B is
        // denied, `evaluate` succeeds (the A branch is acceptable) so the
        // package must NOT be blocked - the user can legitimately choose A.
        // For `A AND B`, `evaluate` fails because B is required, so the
        // package IS blocked.
        return !expr.evaluate(|req| match &req.license {
            spdx::LicenseItem::Spdx { id, .. } => {
                !denied.contains(&id.name.to_ascii_lowercase())
            }
            spdx::LicenseItem::Other(lic_ref) => {
                !denied.contains(&lic_ref.to_string().to_ascii_lowercase())
            }
        });
    }

    let tokens = license_tokens(license);
    tokens.iter().any(|token| denied.contains(token))
}

/// Checks if a license string matches the allowed license policy.
fn license_matches_allowed(license: &str, allowed: &HashSet<String>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    if license.trim().is_empty() {
        return false;
    }

    if let Ok(expr) = spdx::Expression::parse(license) {
        return expr.evaluate(|req| match req.license {
            spdx::LicenseItem::Spdx { id, .. } => {
                allowed.contains(&id.name.to_ascii_lowercase())
            }
            spdx::LicenseItem::Other { .. } => {
                allowed.contains(&license.to_ascii_lowercase())
            }
        });
    }

    let tokens = license_tokens(license);
    !tokens.is_empty() && tokens.iter().any(|token| allowed.contains(token))
}

/// Tokenizes a license string into individual license identifiers.
fn license_tokens(license: &str) -> HashSet<String> {
    license
        .split(|c: char| {
            !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
        })
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

/// Checks the dependency graph for known security vulnerabilities.
///
/// # Errors
///
/// Returns an error if the user aborts the operation after being warned about
/// vulnerabilities, or if system policy blocks vulnerable packages.
pub fn check_for_vulnerabilities(
    graph: &crate::resolver::DependencyGraph,
    yes: bool
) -> Result<()> {
    let mut all_vulnerabilities = Vec::new();

    for node in graph.nodes.values() {
        if let Ok(advisories) = zoi_db::get_advisories_for_package(
            &node.registry_handle,
            &node.pkg.name,
            node.sub_package.as_deref()
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
                        node.sub_package.clone()
                    ));
                }
            }
        }
    }

    if !all_vulnerabilities.is_empty() {
        println!("\n{}", "SECURITY WARNING".red().bold());
        for (adv, version, pkg_name, sub_pkg) in &all_vulnerabilities {
            let display_name = if let Some(sub) = sub_pkg {
                format!("{pkg_name}:{sub}")
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
                    types::Severity::Critical => "Critical".magenta().bold()
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
                "Installation blocked by system policy due to security \
                 vulnerabilities."
            ));
        }

        if !utils::ask_for_confirmation(
            "Do you want to continue with the installation anyway?",
            yes
        ) {
            return Err(anyhow!(
                "Operation aborted by user due to security vulnerabilities."
            ));
        }
    }

    Ok(())
}

/// Performs pre-emptive conflict detection against the filesystem.
///
/// # Errors
///
/// Returns an error if the user aborts the operation after being warned about
/// file conflicts, or if installed packages cannot be retrieved.
///
/// # Panics
///
/// Panics if the internal `all_conflicts` mutex is poisoned.
pub fn check_file_conflicts(
    graph: &crate::resolver::DependencyGraph,
    yes: bool,
    m: &MultiProgress
) -> Result<()> {
    let installed_packages = zoi_resolver::local::get_installed_packages()?;
    let all_conflicts = Mutex::new(HashSet::new());

    let nodes: Vec<&InstallNode> = graph.nodes.values().collect();
    nodes.par_iter().try_for_each(|node| {
        let sub_package_to_check = node.sub_package.as_deref();

        let owned_files: HashSet<String> = installed_packages
            .iter()
            .find(|p| {
                p.name == node.pkg.name
                    && p.sub_package.as_deref() == sub_package_to_check
            })
            .map(|p| p.installed_files.iter().cloned().collect())
            .unwrap_or_default();

        let mut conflicts_for_this_pkg = Vec::new();

        if let Ok(Some(info)) = crate::util::find_prebuilt_info(node) {
            let file_list = db::get_package_files_from_db(
                &node.registry_handle,
                &node.pkg.name,
                node.sub_package.as_deref(),
                &node.pkg.repo
            )
            .unwrap_or(None)
            .or_else(|| {
                info.files_url.as_ref().and_then(|files_url| {
                    crate::util::get_remote_file_list(files_url).ok()
                })
            });

            if let Some(list) = file_list {
                if let Ok(conflicts) = get_conflicts_from_list(
                    list,
                    &node.pkg,
                    sub_package_to_check
                ) {
                    conflicts_for_this_pkg.extend(conflicts);
                }
            } else {
                let archive_filename =
                    info.final_url.split('/').next_back().unwrap_or_default();
                let Ok(archive_cache_root) =
                    zoi_core::cache::get_archive_cache_root()
                else {
                    return Ok(());
                };
                let archive_path = archive_cache_root.join(archive_filename);

                if archive_path.exists()
                    && let Ok(conflicts) = get_file_conflicts_from_archive(
                        &archive_path,
                        &node.pkg,
                        sub_package_to_check
                    )
                {
                    conflicts_for_this_pkg.extend(conflicts);
                }
            }
        }

        for conflict in conflicts_for_this_pkg {
            if !owned_files.contains(&conflict) {
                all_conflicts
                    .lock()
                    .expect("Failed to lock all_conflicts Mutex")
                    .insert(format!(
                        "File '{}' from package '{}' already exists on \
                         filesystem.",
                        conflict, node.pkg.name
                    ));
            }
        }

        Ok::<(), anyhow::Error>(())
    })?;

    let conflicts = all_conflicts
        .into_inner()
        .expect("Failed to get inner value from Mutex");
    if !conflicts.is_empty() {
        m.println(format!("\n{}", "File Conflict Detected:".red().bold()))?;
        for msg in &conflicts {
            m.println(format!("- {msg}"))?;
        }
        if !utils::ask_for_confirmation(
            "\nDo you want to overwrite these files and continue with the \
             installation?",
            yes
        ) {
            return Err(anyhow!(
                "Operation aborted by user due to file conflicts."
            ));
        }
    }

    Ok(())
}

/// Returns a list of files that would conflict with the system.
///
/// # Errors
///
/// Returns an error if the user home directory cannot be determined.
pub fn get_conflicts_from_list(
    list: Vec<String>,
    pkg: &types::Package,
    sub_package_to_check: Option<&str>
) -> Result<Vec<String>> {
    let mut conflicts = Vec::new();
    let sub_prefix = if let Some(sub) = sub_package_to_check {
        format!("data/{sub}/")
    } else {
        "data/".to_string()
    };

    for path_in_archive in list {
        if !path_in_archive.starts_with(&sub_prefix) {
            continue;
        }

        let rel_to_data = &path_in_archive[sub_prefix.len()..];
        let dest_path = if let Some(stripped) =
            rel_to_data.strip_prefix("usrroot/")
        {
            if pkg.scope != types::Scope::System {
                continue;
            }
            Some(zoi_core::sysroot::apply_sysroot(
                PathBuf::from("/").join(stripped)
            ))
        } else if let Some(stripped) = rel_to_data.strip_prefix("usrhome/") {
            utils::get_user_home().map(|h| h.join(stripped))
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

/// Returns a list of files that would conflict with the system by inspecting an
/// archive.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or unpacked.
pub fn get_file_conflicts_from_archive(
    archive_path: &Path,
    pkg: &types::Package,
    sub_package_to_check: Option<&str>
) -> Result<Vec<String>> {
    use std::fs::File;

    use tar::Archive;
    use zstd::stream::read::Decoder as ZstdDecoder;

    let file = File::open(archive_path)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-conflict-check-")
        .tempdir()?;
    archive.unpack(temp_dir.path())?;

    let mut conflicts = Vec::new();
    let data_dir = temp_dir.path().join("data");
    if !data_dir.exists() {
        return Ok(conflicts);
    }

    let subs_to_check = if let Some(sub) = sub_package_to_check {
        vec![sub.to_string()]
    } else {
        vec![String::new()]
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
            let root_dest =
                zoi_core::sysroot::apply_sysroot(PathBuf::from("/"));
            for entry in walkdir::WalkDir::new(&usrroot_src)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .skip(1)
            {
                if entry.file_type().is_file() {
                    let relative_path =
                        entry.path().strip_prefix(&usrroot_src)?;
                    let dest_path = root_dest.join(relative_path);
                    if dest_path.exists() {
                        conflicts.push(dest_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        let usrhome_src = sub_data_dir.join("usrhome");
        if usrhome_src.exists()
            && let Some(home_dest) = utils::get_user_home()
        {
            for entry in walkdir::WalkDir::new(&usrhome_src)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .skip(1)
            {
                if entry.file_type().is_file() {
                    let relative_path =
                        entry.path().strip_prefix(&usrhome_src)?;
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
