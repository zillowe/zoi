use crate::pkg::{local, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[cfg(windows)]
use junction;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let request = resolve::parse_source_string(package_name)?;
    let sub_package = request.sub_package.clone();

    let resolved_source = resolve::resolve_source(package_name, false)?;
    let mut pkg = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().unwrap(),
        None,
        false,
    )?;
    if let Some(repo_name) = resolved_source.repo_name {
        pkg.repo = repo_name;
    }
    let registry_handle = resolved_source.registry_handle;

    let (_manifest, scope) = if let Some(m) =
        local::is_package_installed(&pkg.name, sub_package.as_deref(), types::Scope::User)?
    {
        (m, types::Scope::User)
    } else if let Some(m) =
        local::is_package_installed(&pkg.name, sub_package.as_deref(), types::Scope::System)?
    {
        (m, types::Scope::System)
    } else if let Some(m) =
        local::is_package_installed(&pkg.name, sub_package.as_deref(), types::Scope::Project)?
    {
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
                && version_str != "dependents"
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

    let previous_version_dir = package_dir.join(previous_version.to_string());

    let manifest_filename = if let Some(sub) = &sub_package {
        format!("manifest-{}.yaml", sub)
    } else {
        "manifest.yaml".to_string()
    };
    let prev_manifest_path = previous_version_dir.join(&manifest_filename);
    if !prev_manifest_path.exists() {
        return Err(anyhow!(
            "No manifest found for {} in version {}. Rollback not possible.",
            package_name,
            previous_version
        ));
    }

    let latest_symlink_path = package_dir.join("latest");
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

    let content = fs::read_to_string(&prev_manifest_path)?;
    let prev_manifest: types::InstallManifest = serde_yaml::from_str(&content)?;

    if let Some(bins) = &prev_manifest.bins {
        let bin_root = get_bin_root(scope)?;
        for bin in bins {
            let symlink_path = bin_root.join(bin);
            if symlink_path.exists() || symlink_path.is_symlink() {
                fs::remove_file(&symlink_path)?;
            }

            let mut found_bin = None;
            for entry in WalkDir::new(&previous_version_dir)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() && entry.file_name().to_string_lossy() == *bin {
                    found_bin = Some(entry.path().to_path_buf());
                    break;
                }
            }

            if let Some(target) = found_bin {
                create_symlink(&target, &symlink_path)?;
            }
        }
    }

    let current_version_dir = package_dir.join(current_version.to_string());
    let mut has_other_manifests = false;
    if let Ok(entries) = fs::read_dir(&current_version_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("manifest") && name.ends_with(".yaml") && name != manifest_filename
            {
                has_other_manifests = true;
                break;
            }
        }
    }

    if has_other_manifests {
        for file_path_str in &prev_manifest.installed_files {
            let file_path = PathBuf::from(file_path_str);
            if file_path.exists() {
                if file_path.is_dir() {
                    let _ = fs::remove_dir_all(&file_path);
                } else {
                    let _ = fs::remove_file(&file_path);
                }
            }
        }
        let current_manifest_path = current_version_dir.join(&manifest_filename);
        if current_manifest_path.exists() {
            fs::remove_file(current_manifest_path)?;
        }
    } else {
        fs::remove_dir_all(current_version_dir)?;
    }

    println!(
        "Successfully rolled back '{}' to version {}.",
        package_name.cyan(),
        previous_version.to_string().green()
    );

    Ok(())
}

fn get_bin_root(scope: types::Scope) -> Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(home_dir.join(".zoi/pkgs/bin"))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                Ok(PathBuf::from("/usr/local/bin"))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
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
