use crate::{
    pkg::{hash, local, types},
    project,
};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub fn run() -> Result<()> {
    println!("Verifying project integrity with zoi.lock...");

    let lockfile = project::lockfile::read_zoi_lock()?;
    let installed_packages = local::get_installed_packages()?
        .into_iter()
        .filter(|p| p.scope == types::Scope::Project)
        .collect::<Vec<_>>();

    let mut lockfile_pkgs_map = HashMap::new();
    for (reg_key, pkgs) in &lockfile.details {
        for (short_id, detail) in pkgs {
            let full_id = format!("{}{}", reg_key, short_id);
            lockfile_pkgs_map.insert(full_id, detail);
        }
    }

    let mut installed_pkgs_map = HashMap::new();
    for installed_pkg in &installed_packages {
        let name_with_sub = if let Some(sub) = &installed_pkg.sub_package {
            format!("{}:{}", installed_pkg.name, sub)
        } else {
            installed_pkg.name.clone()
        };
        let full_id = format!(
            "#{}@{}/{}",
            installed_pkg.registry_handle, installed_pkg.repo, name_with_sub
        );
        installed_pkgs_map.insert(full_id, installed_pkg);
    }

    for (full_id, lock_detail) in &lockfile_pkgs_map {
        if let Some(installed_pkg) = installed_pkgs_map.get(full_id) {
            if installed_pkg.version != lock_detail.version {
                return Err(anyhow!(
                    "Version mismatch for '{}': lockfile requires v{}, but v{} is installed.",
                    full_id,
                    lock_detail.version,
                    installed_pkg.version
                ));
            }

            let parts: Vec<&str> = full_id.split('@').collect();
            let registry_handle = parts[0].strip_prefix('#').unwrap();
            let repo_and_name_with_sub = parts[1];

            if let Some(last_slash_idx) = repo_and_name_with_sub.rfind('/') {
                let (repo, name_with_sub) = repo_and_name_with_sub.split_at(last_slash_idx);
                let name_with_sub = &name_with_sub[1..];

                let name = if let Some(colon_idx) = name_with_sub.rfind(':') {
                    &name_with_sub[..colon_idx]
                } else {
                    name_with_sub
                };

                let package_dir =
                    local::get_package_dir(types::Scope::Project, registry_handle, repo, name)?;
                let latest_dir = package_dir.join("latest");
                if !latest_dir.exists() {
                    return Err(anyhow!(
                        "Package '{}' is missing from the project's .zoi directory, though it is in the manifest.",
                        full_id
                    ));
                }
                let integrity = hash::calculate_dir_hash(&latest_dir)?;
                if integrity != lock_detail.integrity {
                    return Err(anyhow!(
                        "Integrity check failed for '{}'. The installed files do not match the lockfile. Your project is in an inconsistent state.",
                        full_id
                    ));
                }
            } else {
                return Err(anyhow!(
                    "Invalid package ID format in lockfile: {}",
                    full_id
                ));
            }
        } else {
            return Err(anyhow!(
                "Package '{}' from zoi.lock is not installed.",
                full_id
            ));
        }
    }

    for full_id in installed_pkgs_map.keys() {
        if !lockfile_pkgs_map.contains_key(full_id) {
            return Err(anyhow!(
                "Package '{}' is installed in the project but is not in zoi.lock.",
                full_id
            ));
        }
    }

    println!("Project is consistent with zoi.lock.");
    Ok(())
}
