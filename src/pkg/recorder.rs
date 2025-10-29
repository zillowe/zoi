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
        home_dir.join(".zoi").join("pkgs").join("zoi.pkgs.json")
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

    let base_package_id = utils::generate_package_id(registry_handle, &pkg.repo);
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

pub fn remove_package_from_record(
    package_name: &str,
    sub_package_name: Option<&str>,
    scope: types::Scope,
) -> Result<()> {
    let mut lockfile = read_lockfile(scope)?;

    let key_to_remove = lockfile
        .packages
        .iter()
        .find(|(_, p)| p.name == package_name && p.sub_package.as_deref() == sub_package_name)
        .map(|(k, _)| k.clone());

    if let Some(key) = key_to_remove
        && lockfile.packages.remove(&key).is_some()
    {
        lockfile.version = env!("CARGO_PKG_VERSION").to_string();
        write_lockfile(&lockfile, scope)?;
    }

    Ok(())
}
