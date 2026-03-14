use crate::pkg::{config, db, local, resolve, types::Scope};
use crate::project;
use anyhow::{Result, anyhow};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

use crate::pkg::plugin::PluginManager;

pub fn run_shim(bin_name: &str, args: Vec<String>, plugin_manager: &PluginManager) -> Result<()> {
    let bin_path = resolve_to_installed_bin(bin_name, plugin_manager)?;

    let mut cmd = Command::new(bin_path);
    cmd.args(args);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(anyhow!("Failed to execute binary '{}': {}", bin_name, err))
    }

    #[cfg(windows)]
    {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        std::process::exit(status.code().unwrap_or(0));
    }
}

pub fn resolve_to_installed_bin(bin_name: &str, plugin_manager: &PluginManager) -> Result<PathBuf> {
    let desired_version = get_desired_version(bin_name, plugin_manager)?;

    let providers = db::find_provides("local", bin_name)?;
    if providers.is_empty() {
        return Err(anyhow!(
            "No installed package provides binary '{}'. Run 'zoi provides {}' to find providers.",
            bin_name,
            bin_name
        ));
    }

    if let Some(version) = &desired_version {
        for (pkg, _) in &providers {
            if let Some(path) = search_store_for_version(&pkg.name, version, bin_name)? {
                return Ok(path);
            }
        }
    }

    let (pkg, _) = &providers[0];
    let version = pkg
        .version
        .as_deref()
        .ok_or_else(|| anyhow!("Package '{}' has no version info in DB", pkg.name))?;

    if let Some(path) = search_store_for_version(&pkg.name, version, bin_name)? {
        return Ok(path);
    }

    for scope in [Scope::Project, Scope::User, Scope::System] {
        let store_root = local::get_store_base_dir(scope)?;
        if !store_root.exists() {
            continue;
        }

        for entry in fs::read_dir(store_root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name) = path.file_name().and_then(|s| s.to_str())
                && dir_name.ends_with(&format!("-{}", pkg.name))
            {
                let latest_dir = path.join("latest");
                if latest_dir.exists()
                    && let Some(p) = find_bin_in_dir(&latest_dir, bin_name)
                {
                    return Ok(p);
                }
            }
        }
    }

    Err(anyhow!(
        "Could not locate binary '{}' in the Zoi store. Try reinstalling the provider package.",
        bin_name
    ))
}

fn get_desired_version(bin_name: &str, plugin_manager: &PluginManager) -> Result<Option<String>> {
    let env_var_name = format!("ZOI_{}_VERSION", bin_name.to_uppercase().replace('-', "_"));
    if let Ok(v) = env::var(&env_var_name) {
        return Ok(Some(v));
    }

    if let Ok(Some(v)) = plugin_manager.trigger_resolve_shim_version(bin_name) {
        return Ok(Some(v));
    }

    if let Ok(project_cfg) = project::config::load() {
        for pkg_spec in project_cfg.pkgs {
            if let Ok(req) = resolve::parse_source_string(&pkg_spec) {
                let is_match = req.name == bin_name || {
                    if let Ok(providers) = db::find_provides("local", bin_name) {
                        providers.iter().any(|(p, _)| p.name == req.name)
                    } else {
                        false
                    }
                };

                if is_match && let Some(v) = req.version_spec {
                    return Ok(Some(v));
                }
            }
        }
    }

    let cfg = config::read_config()?;
    if let Some(v) = cfg.versions.get(bin_name) {
        return Ok(Some(v.clone()));
    }

    Ok(None)
}

fn search_store_for_version(
    pkg_name: &str,
    version: &str,
    bin_name: &str,
) -> Result<Option<PathBuf>> {
    for scope in [Scope::Project, Scope::User, Scope::System] {
        let store_root = local::get_store_base_dir(scope)?;
        if !store_root.exists() {
            continue;
        }

        for entry in fs::read_dir(store_root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name) = path.file_name().and_then(|s| s.to_str())
                && dir_name.ends_with(&format!("-{}", pkg_name))
            {
                let version_dir = path.join(version);
                if version_dir.exists()
                    && let Some(p) = find_bin_in_dir(&version_dir, bin_name)
                {
                    return Ok(Some(p));
                }

                for v_entry in fs::read_dir(&path)? {
                    let v_entry = v_entry?;
                    let v_name = v_entry.file_name().to_string_lossy().to_string();
                    if v_name.starts_with(version) && v_name != "latest" && v_name != "dependents" {
                        let v_dir = path.join(v_name);
                        if let Some(p) = find_bin_in_dir(&v_dir, bin_name) {
                            return Ok(Some(p));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

fn find_bin_in_dir(dir: &std::path::Path, bin_name: &str) -> Option<PathBuf> {
    let bin_path = dir.join("bin").join(bin_name);
    if bin_path.exists() {
        return Some(bin_path);
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() && entry.file_name().to_string_lossy() == bin_name {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}

pub fn create_shim(link_path: &std::path::Path) -> Result<()> {
    let zoi_exe = env::current_exe()?;
    crate::utils::symlink_file(&zoi_exe, link_path)
        .map_err(|e| anyhow!("Failed to create shim: {}", e))
}
