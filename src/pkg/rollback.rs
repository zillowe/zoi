use crate::pkg::{local, types};
use crate::utils;
use colored::*;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn get_rollback_dir(package_name: &str, scope: types::Scope) -> Result<PathBuf, Box<dyn Error>> {
    let store_dir = local::get_store_root(scope)?.join(package_name);
    Ok(store_dir.join("rollback"))
}

pub fn backup_package(package_name: &str, scope: types::Scope) -> Result<(), Box<dyn Error>> {
    println!(
        "Backing up current version of '{}' for rollback...",
        package_name.cyan()
    );

    let store_dir = local::get_store_root(scope)?.join(package_name);
    if !store_dir.exists() {
        println!(
            "{}",
            "No existing installation found to back up. Skipping.".yellow()
        );
        return Ok(());
    }

    let rollback_dir = get_rollback_dir(package_name, scope)?;

    if rollback_dir.exists() {
        fs::remove_dir_all(&rollback_dir)?;
    }
    fs::create_dir_all(&rollback_dir)?;

    let manifest_path = store_dir.join("manifest.yaml");
    if manifest_path.exists() {
        fs::copy(&manifest_path, rollback_dir.join("manifest.yaml"))?;
    }

    let bin_dir = store_dir.join("bin");
    if bin_dir.exists() {
        let rollback_bin_dir = rollback_dir.join("bin");
        fs::create_dir_all(&rollback_bin_dir)?;
        for entry in fs::read_dir(bin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::copy(&path, rollback_bin_dir.join(path.file_name().unwrap()))?;
            }
        }
    }

    println!("{}", "Backup complete.".green());
    Ok(())
}

pub fn run(package_name: &str, yes: bool) -> Result<(), Box<dyn Error>> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let (manifest, scope) =
        if let Some(m) = local::is_package_installed(package_name, types::Scope::User)? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(package_name, types::Scope::System)? {
            (m, types::Scope::System)
        } else {
            return Err(format!("Package '{}' is not installed.", package_name).into());
        };

    let rollback_dir = get_rollback_dir(package_name, scope)?;
    if !rollback_dir.exists() {
        return Err(format!("No rollback version found for '{}'.", package_name).into());
    }

    let rollback_manifest_path = rollback_dir.join("manifest.yaml");
    if !rollback_manifest_path.exists() {
        return Err("Rollback manifest not found.".into());
    }

    let content = fs::read_to_string(&rollback_manifest_path)?;
    let rollback_manifest: types::InstallManifest = serde_yaml::from_str(&content)?;

    println!(
        "Rolling back from version {} to {}",
        manifest.version.yellow(),
        rollback_manifest.version.green()
    );

    if !utils::ask_for_confirmation("This will uninstall the current version. Continue?", yes) {
        println!("Operation aborted.");
        return Ok(());
    }

    println!("Uninstalling current version...");
    let store_dir = local::get_store_root(scope)?.join(package_name);

    if let Some(bins) = &manifest.bins {
        let bin_root = get_bin_root()?;
        for bin in bins {
            let symlink_path = bin_root.join(bin);
            if symlink_path.exists() {
                if let Ok(target) = fs::read_link(&symlink_path) {
                    if target.starts_with(&store_dir) {
                        fs::remove_file(&symlink_path)?;
                    }
                }
            }
        }
    } else {
        let symlink_path = get_bin_root()?.join(package_name);
        if symlink_path.exists() {
            if let Ok(target) = fs::read_link(&symlink_path) {
                if target.starts_with(&store_dir) {
                    fs::remove_file(symlink_path)?;
                }
            }
        }
    }

    if store_dir.exists() {
        fs::remove_dir_all(&store_dir)?;
    }

    println!("Restoring previous version...");
    fs::rename(&rollback_dir, &store_dir)?;

    let store_bin_dir = store_dir.join("bin");
    if let Some(bins) = &rollback_manifest.bins {
        let bin_root = get_bin_root()?;
        for bin_name in bins {
            let mut bin_path_in_store = store_bin_dir.join(bin_name);
            if !bin_path_in_store.exists() {
                bin_path_in_store = store_bin_dir.join(format!("{}.exe", bin_name));
                if !bin_path_in_store.exists() {
                    eprintln!(
                        "Warning: Binary '{}' not found in restored package.",
                        bin_name
                    );
                    continue;
                }
            }
            create_symlink(&bin_path_in_store, &bin_root.join(bin_name))?;
        }
    } else {
        let bin_root = get_bin_root()?;
        let mut bin_path_in_store = store_bin_dir.join(package_name);
        if !bin_path_in_store.exists() {
            bin_path_in_store = store_bin_dir.join(format!("{}.exe", package_name));
        }
        if bin_path_in_store.exists() {
            create_symlink(&bin_path_in_store, &bin_root.join(package_name))?;
        }
    }

    println!(
        "Successfully rolled back '{}' to version {}.",
        package_name.cyan(),
        rollback_manifest.version.green()
    );

    Ok(())
}

fn get_bin_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("bin"))
}

fn create_symlink(target: &Path, link: &Path) -> Result<(), Box<dyn Error>> {
    if link.exists() {
        fs::remove_file(link)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(windows)]
    {
        fs::copy(target, link)?;
    }
    Ok(())
}
