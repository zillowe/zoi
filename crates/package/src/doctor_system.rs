use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// Implements proactive "System Health Checks" for the Zoi environment.
///
/// These checks (run via `zoi doctor`) help users identify and fix:
/// - Orphaned Packages: Dependencies no longer required by any package.
/// - Broken Symlinks: Stale binary shims in the PATH.
/// - Integrity Issues: Mismatches between the store and the lockfile.
/// - Path Configuration: Missing Zoi binary directories in the host PATH.
/// - Registry Staleness: Repositories that haven't been synced recently.
use anyhow::{Result, anyhow};
use rayon::prelude::*;
use walkdir::WalkDir;
use zoi_core::types::{InstallReason, Scope};
use zoi_core::{config, pgp, recorder, sysroot, utils};
use zoi_resolver::{local, resolve};

/// Returns the root directory for binary shims for a given scope.
fn get_bin_root(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::User => {
            let home_dir = utils::get_user_home()
                .ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(sysroot::apply_sysroot(home_dir.join(".zoi/pkgs/bin")))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(sysroot::apply_sysroot(PathBuf::from(
                    "C:\\ProgramData\\zoi\\pkgs\\bin"
                )))
            } else {
                Ok(sysroot::apply_sysroot(PathBuf::from("/usr/local/bin")))
            }
        }
        Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

/// Checks for broken symlinks in all binary shim directories.
///
/// Returns a list of paths to symlinks that point to non-existent locations.
///
/// # Errors
///
/// Returns an error if any of the binary shim directories cannot be read.
pub fn check_broken_symlinks() -> Result<Vec<PathBuf>> {
    let scopes = [Scope::User, Scope::System, Scope::Project];

    let broken_links: Vec<PathBuf> = scopes
        .into_par_iter()
        .map(|scope| {
            let mut links = Vec::new();
            if let Ok(root) = get_bin_root(scope)
                && root.exists()
                && let Ok(entries) = fs::read_dir(root)
            {
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type()
                        && ft.is_symlink()
                    {
                        let path = entry.path();
                        if !path.exists() {
                            links.push(path);
                        }
                    }
                }
            }
            links
        })
        .flatten()
        .collect();

    Ok(broken_links)
}

/// Checks if Zoi's binary directory is in the system PATH.
///
/// Returns a warning message if the directory is missing from PATH.
///
/// # Errors
///
/// Returns an error if the user's home directory cannot be found.
pub fn check_path_configuration() -> Result<Option<String>> {
    if let Some(home) = utils::get_user_home() {
        let zoi_bin_dir =
            sysroot::apply_sysroot(home.join(".zoi").join("pkgs").join("bin"));
        if !zoi_bin_dir.exists() {
            return Ok(None);
        }

        if let Ok(path_var) = std::env::var("PATH")
            && !std::env::split_paths(&path_var).any(|p| p == zoi_bin_dir)
        {
            return Ok(Some(format!(
                "Zoi's user binary directory ({}) is not in your PATH.",
                zoi_bin_dir.display()
            )));
        }
    }
    Ok(None)
}

/// Checks if any repositories are significantly out of date.
///
/// Returns a warning message if the default registry hasn't been synced in over
/// a week.
///
/// # Errors
///
/// Returns an error if the database root cannot be resolved or if the
/// repository metadata cannot be read.
pub fn check_outdated_repos() -> Result<Option<String>> {
    let db_root = sysroot::apply_sysroot(resolve::get_db_root()?);
    let config = config::read_config()?;

    if let Some(default_reg) = config.default_registry
        && !default_reg.handle.is_empty()
    {
        let repo_path = db_root.join(default_reg.handle);
        let fetch_head = repo_path.join(".git/FETCH_HEAD");
        if fetch_head.exists() {
            let metadata = fs::metadata(fetch_head)?;
            if let Ok(modified) = metadata.modified()
                && let Ok(since_modified) =
                    SystemTime::now().duration_since(modified)
                && since_modified.as_secs() > 60 * 60 * 24 * 7
            {
                let days = since_modified.as_secs() / (60 * 60 * 24);
                return Ok(Some(format!(
                    "Default repository has not been synced in over a week \
                     (last sync: {days} days ago)."
                )));
            }
        } else if repo_path.join(".git").exists() {
            return Ok(Some(
                "Default repository has never been synced.".to_string()
            ));
        }
    }

    Ok(None)
}

/// Finds packages that are defined in multiple registries.
///
/// Returns a list of package IDs and the registries they are defined in.
///
/// # Errors
///
/// Returns an error if the database root cannot be resolved or if the registry
/// directories cannot be read.
pub fn check_duplicate_packages() -> Result<Vec<(String, Vec<String>)>> {
    let db_root = sysroot::apply_sysroot(resolve::get_db_root()?);
    if !db_root.exists() {
        return Ok(Vec::new());
    }

    let mut package_map: HashMap<String, Vec<String>> = HashMap::new();

    if let Ok(entries) = fs::read_dir(&db_root) {
        for entry in entries.flatten() {
            let registry_handle =
                entry.file_name().to_string_lossy().to_string();
            if !entry.path().is_dir()
                || registry_handle.starts_with('.')
                || registry_handle == "git"
            {
                continue;
            }

            for pkg_entry in WalkDir::new(entry.path())
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name().to_string_lossy().ends_with(".pkg.lua")
                })
            {
                let pkg_path = pkg_entry.path();
                if let Ok(rel_path) = pkg_path.strip_prefix(entry.path()) {
                    let pkg_id = rel_path
                        .to_string_lossy()
                        .to_string()
                        .replace('\\', "/");
                    package_map
                        .entry(pkg_id)
                        .or_default()
                        .push(registry_handle.clone());
                }
            }
        }
    }

    let mut duplicates: Vec<_> = package_map
        .into_iter()
        .filter(|(_, registries)| registries.len() > 1)
        .collect();
    duplicates.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(duplicates)
}

/// Checks if PGP keys required by security policy are present in the keyring.
///
/// Returns a list of missing key fingerprints.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read.
pub fn check_pgp_configuration() -> Result<Vec<String>> {
    let config = config::read_config()?;
    let mut missing_keys = Vec::new();

    if let Some(enforcement) = config.policy.signature_enforcement
        && enforcement.enable
    {
        for key in enforcement.trusted_keys {
            match pgp::get_certs_by_name_or_fingerprint(std::slice::from_ref(
                &key
            )) {
                Ok(certs) if certs.is_empty() => {
                    missing_keys.push(key);
                }
                Err(_) => {
                    missing_keys.push(key);
                }
                _ => {}
            }
        }
    }

    Ok(missing_keys)
}

/// Validates that all packages recorded in the lockfile are actually installed.
///
/// Returns a list of names for missing packages.
///
/// # Errors
///
/// Returns an error if the lockfile cannot be read.
pub fn validate_lockfile_integrity() -> Result<Vec<String>> {
    let recorded_packages = recorder::get_recorded_packages()?;

    let missing_packages: Vec<String> = recorded_packages
        .into_par_iter()
        .filter_map(|pkg_record| {
            let manifest = local::is_package_installed(
                &pkg_record.name,
                pkg_record.sub_package.as_deref(),
                Scope::User
            )
            .ok()
            .flatten()
            .or_else(|| {
                local::is_package_installed(
                    &pkg_record.name,
                    pkg_record.sub_package.as_deref(),
                    Scope::System
                )
                .ok()
                .flatten()
            })
            .or_else(|| {
                local::is_package_installed(
                    &pkg_record.name,
                    pkg_record.sub_package.as_deref(),
                    Scope::Project
                )
                .ok()
                .flatten()
            });

            if manifest.is_none() {
                let name = if let Some(sub) = pkg_record.sub_package {
                    format!("{}:{}", pkg_record.name, sub)
                } else {
                    pkg_record.name
                };
                Some(name)
            } else {
                None
            }
        })
        .collect();

    Ok(missing_packages)
}

/// Finds orphaned packages (dependencies no longer required by any package).
///
/// Returns a list of names for orphaned packages.
///
/// # Errors
///
/// Returns an error if the installed packages cannot be retrieved.
pub fn check_orphaned_packages() -> Result<Vec<String>> {
    let all_installed = local::get_installed_packages()?;

    let orphaned: Vec<String> = all_installed
        .into_par_iter()
        .filter_map(|package| {
            if !matches!(package.reason, InstallReason::Dependency { .. }) {
                return None;
            }

            let package_dir = local::get_package_dir(
                package.scope,
                &package.registry_handle,
                &package.repo,
                &package.name
            )
            .ok()?;

            let dependents = local::get_dependents(&package_dir).ok()?;

            if dependents.is_empty() {
                let name = if let Some(sub) = package.sub_package {
                    format!("{}:{}", package.name, sub)
                } else {
                    package.name
                };
                Some(name)
            } else {
                None
            }
        })
        .collect();

    Ok(orphaned)
}

/// Finds "ghost" dependent links (links from packages that are no longer
/// installed).
///
/// Returns a list of (path, `parent_id`) pairs for stale dependent links.
///
/// # Errors
///
/// Returns an error if the store directories or dependent files cannot be read.
pub fn check_ghost_dependents() -> Result<Vec<(PathBuf, String)>> {
    let scopes = [Scope::User, Scope::System, Scope::Project];
    let mut ghost_links = Vec::new();

    let all_installed = local::get_installed_packages()?;
    let mut installed_ids = HashSet::new();
    for manifest in all_installed {
        let full_id = format!(
            "#{}@{}/{}@{}",
            manifest.registry_handle,
            manifest.repo,
            manifest.name,
            manifest.version
        );
        installed_ids.insert(full_id);

        if let Some(sub) = manifest.sub_package {
            let full_id_sub = format!(
                "#{}@{}/{}:{}@{}",
                manifest.registry_handle,
                manifest.repo,
                manifest.name,
                sub,
                manifest.version
            );
            installed_ids.insert(full_id_sub);
        }
    }

    for scope in scopes {
        if let Ok(store_root) = local::get_store_base_dir(scope)
            && store_root.exists()
        {
            for entry in fs::read_dir(store_root)? {
                let path = entry?.path();
                if path.is_dir() {
                    let dependents_dir = path.join("dependents");
                    if dependents_dir.exists() {
                        for dep_entry in fs::read_dir(dependents_dir)? {
                            let dep_path = dep_entry?.path();
                            if dep_path.is_file()
                                && let Some(file_name) = dep_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                && let Ok(decoded) = hex::decode(file_name)
                                && let Ok(parent_id) =
                                    String::from_utf8(decoded)
                                && !installed_ids.contains(&parent_id)
                            {
                                ghost_links.push((dep_path, parent_id));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ghost_links)
}

/// Removes stale dependent links identified by `check_ghost_dependents`.
///
/// # Errors
///
/// Returns an error if any of the ghost links cannot be removed.
pub fn prune_ghost_dependents(ghost_links: &[(PathBuf, String)]) -> Result<()> {
    for (path, _) in ghost_links {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Result of checking for external tool dependencies.
pub struct ToolCheckResult {
    /// List of essential tools (e.g. git) that are missing.
    pub essential_missing: Vec<String>,
    /// List of recommended tools (e.g. bwrap) that are missing.
    pub recommended_missing: Vec<String>
}

/// Checks if essential and recommended external tools are available in PATH.
pub fn check_external_tools() -> ToolCheckResult {
    let mut essential_missing = Vec::new();
    let mut recommended_missing = Vec::new();

    let essential = ["git", "gpg"];
    let recommended = ["bwrap"];

    for tool in essential {
        if !utils::command_exists(tool) {
            essential_missing.push(tool.to_string());
        }
    }

    for tool in recommended {
        if !utils::command_exists(tool) {
            recommended_missing.push(tool.to_string());
        }
    }

    ToolCheckResult {
        essential_missing,
        recommended_missing
    }
}
