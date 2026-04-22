use crate::pkg::{types, utils};
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn get_lockfile_path(scope: types::Scope) -> Result<PathBuf> {
    let path = if scope == types::Scope::Project {
        std::env::current_dir()?
            .join(".zoi")
            .join("pkgs")
            .join("zoi.pkgs.json")
    } else {
        let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
        crate::pkg::sysroot::apply_sysroot(home_dir.join(".zoi").join("pkgs").join("zoi.pkgs.json"))
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn read_lockfile(scope: types::Scope) -> Result<types::Lockfile> {
    let path = get_lockfile_path(scope)?;
    if !path.exists() || fs::read_to_string(&path)?.trim().is_empty() {
        return Ok(types::Lockfile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            packages: HashMap::new(),
        });
    }
    let content = fs::read_to_string(path)?;
    let lockfile = serde_json::from_str(&content)?;
    Ok(lockfile)
}

fn write_lockfile(lockfile: &types::Lockfile, scope: types::Scope) -> Result<()> {
    let path = get_lockfile_path(scope)?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn record_package(
    pkg: &types::Package,
    reason: &types::InstallReason,
    installed_dependencies: &[String],
    registry_handle: &str,
    chosen_options: &[String],
    chosen_optionals: &[String],
    sub_package: Option<String>,
) -> Result<()> {
    let mut lockfile = read_lockfile(pkg.scope)?;

    let base_package_id = utils::generate_package_id(registry_handle, &pkg.repo, &pkg.name);
    let package_id = if let Some(sub) = &sub_package {
        format!("{}:{}", base_package_id, sub)
    } else {
        base_package_id
    };

    let lockfile_pkg = types::LockfilePackage {
        name: pkg.name.clone(),
        sub_package,
        repo: pkg.repo.clone(),
        registry: registry_handle.to_string(),
        version: pkg
            .version
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("Missing version"))?,
        date: Utc::now().to_rfc3339(),
        reason: reason.clone(),
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        conflicts: pkg.conflicts.clone(),
        replaces: pkg.replaces.clone(),
        provides: pkg.provides.clone(),
        backup: pkg.backup.clone(),
        dependencies: installed_dependencies.to_vec(),
        chosen_options: chosen_options.to_vec(),
        chosen_optionals: chosen_optionals.to_vec(),
    };

    lockfile.packages.insert(package_id, lockfile_pkg);
    lockfile.version = env!("CARGO_PKG_VERSION").to_string();

    write_lockfile(&lockfile, pkg.scope)
}

pub fn update_package_reason(
    manifest: &types::InstallManifest,
    new_reason: types::InstallReason,
) -> Result<()> {
    let mut lockfile = read_lockfile(manifest.scope)?;
    let package_id = crate::pkg::utils::generate_package_id(
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
    );
    let package_id = if let Some(sub) = &manifest.sub_package {
        format!("{}:{}", package_id, sub)
    } else {
        package_id
    };

    if let Some(pkg) = lockfile.packages.get_mut(&package_id) {
        pkg.reason = new_reason;
        lockfile.version = env!("CARGO_PKG_VERSION").to_string();
        write_lockfile(&lockfile, manifest.scope)?;
        Ok(())
    } else {
        Err(anyhow!("Package '{}' not found in record.", manifest.name))
    }
}

pub fn remove_package_from_record(manifest: &types::InstallManifest) -> Result<()> {
    let mut lockfile = read_lockfile(manifest.scope)?;
    let package_id = crate::pkg::utils::generate_package_id(
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
    );
    let package_id = if let Some(sub) = &manifest.sub_package {
        format!("{}:{}", package_id, sub)
    } else {
        package_id
    };

    if lockfile.packages.remove(&package_id).is_some() {
        lockfile.version = env!("CARGO_PKG_VERSION").to_string();
        write_lockfile(&lockfile, manifest.scope)?;
    }

    Ok(())
}

pub fn get_recorded_packages() -> Result<Vec<types::LockfilePackage>> {
    let mut all_packages = Vec::new();
    for scope in [
        types::Scope::User,
        types::Scope::System,
        types::Scope::Project,
    ] {
        if let Ok(lockfile) = read_lockfile(scope) {
            all_packages.extend(lockfile.packages.into_values());
        }
    }
    Ok(all_packages)
}
