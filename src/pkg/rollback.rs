use crate::pkg::{local, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use junction;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let resolved_source = resolve::resolve_source(package_name)?;
    let mut pkg =
        crate::pkg::lua::parser::parse_lua_package(resolved_source.path.to_str().unwrap(), None)?;
    if let Some(repo_name) = resolved_source.repo_name {
        pkg.repo = repo_name;
    }
    let registry_handle = resolved_source.registry_handle;

    let (_manifest, scope) =
        if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::User)? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::System)? {
            (m, types::Scope::System)
        } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::Project)? {
            (m, types::Scope::Project)
        } else {
            return Err(anyhow!("Package '{}' is not installed.", package_name));
        };

    let handle = registry_handle.as_deref().unwrap_or("local");
    let package_dir = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;

    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&package_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(version_str) = path.file_name().and_then(|s| s.to_str())
                && version_str != "latest"
                && let Ok(version) = Version::parse(version_str)
            {
                versions.push(version);
            }
        }
    }
    versions.sort();

    if versions.len() < 2 {
        return Err(anyhow!("No previous version to roll back to."));
    }

    let current_version = versions.pop().unwrap();
    let previous_version = versions.pop().unwrap();

    println!(
        "Rolling back from version {} to {}",
        current_version.to_string().yellow(),
        previous_version.to_string().green()
    );

    if !utils::ask_for_confirmation("This will remove the current version. Continue?", yes) {
        println!("Operation aborted.");
        return Ok(());
    }

    let latest_symlink_path = package_dir.join("latest");
    let previous_version_dir = package_dir.join(previous_version.to_string());
    if latest_symlink_path.exists() || latest_symlink_path.is_symlink() {
        if latest_symlink_path.is_dir() {
            fs::remove_dir_all(&latest_symlink_path)?;
        } else {
            fs::remove_file(&latest_symlink_path)?;
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&previous_version_dir, &latest_symlink_path)?;
    #[cfg(windows)]
    {
        junction::create(&previous_version_dir, &latest_symlink_path)?;
    }

    let prev_manifest_path = previous_version_dir.join("manifest.yaml");
    let content = fs::read_to_string(&prev_manifest_path)?;
    let prev_manifest: types::InstallManifest = serde_yaml::from_str(&content)?;

    if let Some(bins) = &prev_manifest.bins {
        let bin_root = get_bin_root()?;
        for bin in bins {
            let symlink_path = bin_root.join(bin);
            if symlink_path.exists() {
                fs::remove_file(&symlink_path)?;
            }
            let bin_path_in_store = previous_version_dir.join("bin").join(bin);
            if bin_path_in_store.exists() {
                create_symlink(&bin_path_in_store, &symlink_path)?;
            }
        }
    }

    let current_version_dir = package_dir.join(current_version.to_string());
    fs::remove_dir_all(current_version_dir)?;

    println!(
        "Successfully rolled back '{}' to version {}.",
        package_name.cyan(),
        previous_version.to_string().green()
    );

    Ok(())
}

fn get_bin_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("bin"))
}

fn create_symlink(target: &Path, link: &Path) -> Result<()> {
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
