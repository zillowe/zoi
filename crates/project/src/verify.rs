use anyhow::{Result, anyhow};
use std::collections::HashMap;
use zoi_core::{hash, types};
use zoi_resolver::local;

pub fn run() -> Result<()> {
    println!("Verifying project integrity with zoi.lock...");

    let lockfile = crate::lockfile::read_zoi_lock()?;
    let installed_packages = local::get_installed_packages()?
        .into_iter()
        .filter(|p| p.scope == types::Scope::Project)
        .collect::<Vec<_>>();

    let mut lockfile_pkgs_map = HashMap::new();
    for (pkg_key, detail) in &lockfile.installed_packages {
        lockfile_pkgs_map.insert(pkg_key.clone(), detail);
    }

    let mut installed_pkgs_map = HashMap::new();
    for installed_pkg in &installed_packages {
        let pkg_key = if let Some(sub) = &installed_pkg.sub_package {
            format!("@{}/{}:{}", installed_pkg.repo, installed_pkg.name, sub)
        } else {
            format!("@{}/{}", installed_pkg.repo, installed_pkg.name)
        };
        installed_pkgs_map.insert(pkg_key, installed_pkg);
    }

    for (pkg_key, lock_detail) in &lockfile_pkgs_map {
        if let Some(installed_pkg) = installed_pkgs_map.get(pkg_key) {
            if installed_pkg.version != lock_detail.version {
                return Err(anyhow!(
                    "Version mismatch for '{}': lockfile requires v{}, but v{} is installed.",
                    pkg_key,
                    lock_detail.version,
                    installed_pkg.version
                ));
            }

            let package_dir = local::get_package_dir(
                types::Scope::Project,
                &lock_detail.registry,
                &lock_detail.repo,
                &installed_pkg.name,
            )?;
            let version_dir = package_dir.join(&lock_detail.version);
            if !version_dir.exists() {
                return Err(anyhow!(
                    "Package '{}' is missing from the project's .zoi directory, though it is in the manifest.",
                    pkg_key
                ));
            }
            let integrity = hash::calculate_dir_hash(&version_dir)?;
            let lock_hash_only = lock_detail
                .hash
                .strip_prefix("sha512-")
                .unwrap_or(&lock_detail.hash);
            if integrity != lock_hash_only {
                return Err(anyhow!(
                    "Integrity check failed for '{}'. The installed files do not match the lockfile. Your project is in an inconsistent state.",
                    pkg_key
                ));
            }
        } else {
            return Err(anyhow!(
                "Package '{}' from zoi.lock is not installed.",
                pkg_key
            ));
        }
    }

    for pkg_key in installed_pkgs_map.keys() {
        if !lockfile_pkgs_map.contains_key(pkg_key) {
            return Err(anyhow!(
                "Package '{}' is installed in the project but is not in zoi.lock.",
                pkg_key
            ));
        }
    }

    println!("Project is consistent with zoi.lock.");
    Ok(())
}
