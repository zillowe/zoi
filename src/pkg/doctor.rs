use crate::pkg::types::Scope;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use walkdir::WalkDir;

fn get_bin_root(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(crate::pkg::sysroot::apply_sysroot(
                home_dir.join(".zoi/pkgs/bin"),
            ))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(crate::pkg::sysroot::apply_sysroot(PathBuf::from(
                    "C:\\ProgramData\\zoi\\pkgs\\bin",
                )))
            } else {
                Ok(crate::pkg::sysroot::apply_sysroot(PathBuf::from(
                    "/usr/local/bin",
                )))
            }
        }
        Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

use rayon::prelude::*;

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

pub fn check_path_configuration() -> Result<Option<String>> {
    if let Some(home) = home::home_dir() {
        let zoi_bin_dir =
            crate::pkg::sysroot::apply_sysroot(home.join(".zoi").join("pkgs").join("bin"));
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

pub fn check_outdated_repos() -> Result<Option<String>> {
    let db_root = crate::pkg::sysroot::apply_sysroot(crate::pkg::resolve::get_db_root()?);
    let config = crate::pkg::config::read_config()?;

    if let Some(default_reg) = config.default_registry
        && !default_reg.handle.is_empty()
    {
        let repo_path = db_root.join(default_reg.handle);
        let fetch_head = repo_path.join(".git/FETCH_HEAD");
        if fetch_head.exists() {
            let metadata = fs::metadata(fetch_head)?;
            if let Ok(modified) = metadata.modified()
                && let Ok(since_modified) = SystemTime::now().duration_since(modified)
                && since_modified.as_secs() > 60 * 60 * 24 * 7
            {
                return Ok(Some(format!(
                    "Default repository has not been synced in over a week (last sync: {} days ago).",
                    since_modified.as_secs() / (60 * 60 * 24)
                )));
            }
        } else if repo_path.join(".git").exists() {
            return Ok(Some(
                "Default repository has never been synced.".to_string(),
            ));
        }
    }

    Ok(None)
}

pub fn check_duplicate_packages() -> Result<Vec<(String, Vec<String>)>> {
    let db_root = crate::pkg::sysroot::apply_sysroot(crate::pkg::resolve::get_db_root()?);
    if !db_root.exists() {
        return Ok(Vec::new());
    }

    let mut package_map: HashMap<String, Vec<String>> = HashMap::new();

    if let Ok(entries) = fs::read_dir(&db_root) {
        for entry in entries.flatten() {
            let registry_handle = entry.file_name().to_string_lossy().to_string();
            if !entry.path().is_dir()
                || registry_handle.starts_with('.')
                || registry_handle == "git"
            {
                continue;
            }

            for pkg_entry in WalkDir::new(entry.path())
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".pkg.lua"))
            {
                let pkg_path = pkg_entry.path();
                if let Ok(rel_path) = pkg_path.strip_prefix(entry.path()) {
                    let pkg_id = rel_path.to_string_lossy().to_string().replace('\\', "/");
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

pub fn check_pgp_configuration() -> Result<Vec<String>> {
    let config = crate::pkg::config::read_config()?;
    let mut missing_keys = Vec::new();

    if let Some(enforcement) = config.policy.signature_enforcement
        && enforcement.enable
    {
        for key in enforcement.trusted_keys {
            match crate::pkg::pgp::get_certs_by_name_or_fingerprint(std::slice::from_ref(&key)) {
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

pub fn validate_pkgs_json_integrity() -> Result<Vec<String>> {
    let recorded_packages = crate::pkg::recorder::get_recorded_packages()?;

    let missing_packages: Vec<String> = recorded_packages
        .into_par_iter()
        .filter_map(|pkg_record| {
            let manifest = crate::pkg::local::is_package_installed(
                &pkg_record.name,
                pkg_record.sub_package.as_deref(),
                Scope::User,
            )
            .ok()
            .flatten()
            .or_else(|| {
                crate::pkg::local::is_package_installed(
                    &pkg_record.name,
                    pkg_record.sub_package.as_deref(),
                    Scope::System,
                )
                .ok()
                .flatten()
            })
            .or_else(|| {
                crate::pkg::local::is_package_installed(
                    &pkg_record.name,
                    pkg_record.sub_package.as_deref(),
                    Scope::Project,
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

pub fn check_orphaned_packages() -> Result<Vec<String>> {
    let all_installed = crate::pkg::local::get_installed_packages()?;

    let orphaned: Vec<String> = all_installed
        .into_par_iter()
        .filter_map(|package| {
            if !matches!(
                package.reason,
                crate::pkg::types::InstallReason::Dependency { .. }
            ) {
                return None;
            }

            let package_dir = crate::pkg::local::get_package_dir(
                package.scope,
                &package.registry_handle,
                &package.repo,
                &package.name,
            )
            .ok()?;

            let dependents = crate::pkg::local::get_dependents(&package_dir).ok()?;

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

pub struct ToolCheckResult {
    pub essential_missing: Vec<String>,
    pub recommended_missing: Vec<String>,
}

pub fn check_external_tools() -> ToolCheckResult {
    let mut essential_missing = Vec::new();
    let mut recommended_missing = Vec::new();

    let essential = ["git"];
    let recommended = ["less", "gpg"];

    for tool in essential {
        if !crate::utils::command_exists(tool) {
            essential_missing.push(tool.to_string());
        }
    }

    for tool in recommended {
        if !crate::utils::command_exists(tool) {
            recommended_missing.push(tool.to_string());
        }
    }

    ToolCheckResult {
        essential_missing,
        recommended_missing,
    }
}
