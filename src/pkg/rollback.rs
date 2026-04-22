use crate::pkg::{local, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use semver::Version;
use std::fs;
use std::path::PathBuf;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    println!("Attempting to roll back '{}'...", package_name.cyan());

    let request = resolve::parse_source_string(package_name)?;
    let sub_package = request.sub_package.clone();
    let scope_order = [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project,
    ];
    let mut current_manifest = None;
    let mut scope = None;
    for candidate_scope in scope_order {
        let mut matches = local::find_installed_manifests_matching(&request, candidate_scope)?;
        match matches.len() {
            0 => continue,
            1 => {
                current_manifest = Some(matches.remove(0));
                scope = Some(candidate_scope);
                break;
            }
            _ => {
                return Err(anyhow!(
                    "Package '{}' is ambiguous in {:?} scope. Use an explicit source like '#handle@repo/name[:sub]@version'.",
                    request.name,
                    candidate_scope
                ));
            }
        }
    }

    let Some(current_manifest) = current_manifest else {
        return Err(anyhow!("Package '{}' is not installed.", package_name));
    };
    let scope = scope.expect("scope should be set when a manifest is found");

    let package_dir = local::get_package_dir(
        scope,
        &current_manifest.registry_handle,
        &current_manifest.repo,
        &current_manifest.name,
    )?;

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

    let current_version = versions.pop().expect("Versions length checked above");
    let previous_version = versions.pop().expect("Versions length checked above");

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
    utils::symlink_dir(&previous_version_dir, &latest_symlink_path)?;

    let content = fs::read_to_string(&prev_manifest_path)?;
    let prev_manifest: types::InstallManifest = serde_yaml::from_str(&content)?;

    if let Some(bins) = &prev_manifest.bins {
        let bin_root = get_bin_root(scope)?;
        for bin in bins {
            let symlink_path = bin_root.join(bin);
            crate::pkg::shim::create_shim(&symlink_path)?;
        }
    } else if prev_manifest.sub_package.is_none() {
        let symlink_path = get_bin_root(scope)?.join(&current_manifest.name);
        crate::pkg::shim::create_shim(&symlink_path)?;
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
        for file_path_str in &current_manifest.installed_files {
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
            Ok(crate::pkg::sysroot::apply_sysroot(
                home_dir.join(".zoi/pkgs/bin"),
            ))
        }
        types::Scope::System => {
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
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}
