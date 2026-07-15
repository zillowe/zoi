use anyhow::{Result, anyhow};
use std::collections::HashMap;
use zoi_core::{hash, types};
use zoi_resolver::local;

/// Verifies the cryptographic integrity of the project's local environment.
///
/// This is a critical security and reproducibility check in Specification v2.
/// It performs a three-way cross-reference between:
/// - `zoi.lock`: The record of what SHOULD be installed and its SHA-512 hash.
/// - `manifest.yaml`: The record of what IS recorded as installed in the store.
/// - Filesystem: The actual presence and content of files on disk.
///
/// If any file in the store has been tampered with or if a manual change was made
/// that isn't reflected in the lockfile, this function will return an error.
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
            format!(
                "@{}/{}:{}",
                installed_pkg.repo.trim(),
                installed_pkg.name.trim(),
                sub.trim()
            )
        } else {
            format!(
                "@{}/{}",
                installed_pkg.repo.trim(),
                installed_pkg.name.trim()
            )
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
                let manifest_filename = if let Some(sub) = &lock_detail.sub_package {
                    format!("manifest-{}.yaml", sub)
                } else {
                    "manifest.yaml".to_string()
                };
                let manifest_path = version_dir.join(manifest_filename);

                return Err(anyhow!(
                    "Integrity check failed for '{}'. The installed files do not match the lockfile.\n\
                     Expected hash: {}\n\
                     Actual hash:   {}\n\
                     Manifest path: {}\n\
                     Your project is in an inconsistent state.",
                    pkg_key,
                    lock_hash_only,
                    integrity,
                    manifest_path.display()
                ));
            }
        } else {
            let hex_key = pkg_key
                .as_bytes()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join("");
            return Err(anyhow!(
                "Package '{}' (hex: {}) from zoi.lock is not installed.",
                pkg_key,
                hex_key
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
