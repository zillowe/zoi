use crate::pkg::{types, utils};
use chrono::Utc;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn get_lockfile_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let path = home_dir.join(".zoi").join("pkgs").join("zoi.pkgs.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn read_lockfile() -> Result<types::Lockfile, Box<dyn Error>> {
    let path = get_lockfile_path()?;
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

fn write_lockfile(lockfile: &types::Lockfile) -> Result<(), Box<dyn Error>> {
    let path = get_lockfile_path()?;
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
) -> Result<(), Box<dyn Error>> {
    let mut lockfile = read_lockfile()?;

    let package_id = utils::generate_package_id(registry_handle, &pkg.repo);

    let lockfile_pkg = types::LockfilePackage {
        name: pkg.name.clone(),
        repo: pkg.repo.clone(),
        registry: registry_handle.to_string(),
        version: pkg.version.as_ref().cloned().ok_or("Missing version")?,
        date: Utc::now().to_rfc3339(),
        reason: reason.clone(),
        scope: pkg.scope,
        bins: pkg.bins.clone(),
        conflicts: pkg.conflicts.clone(),
        dependencies: installed_dependencies.to_vec(),
        chosen_options: chosen_options.to_vec(),
        chosen_optionals: chosen_optionals.to_vec(),
    };

    lockfile.packages.insert(package_id, lockfile_pkg);
    lockfile.version = env!("CARGO_PKG_VERSION").to_string();

    write_lockfile(&lockfile)
}

pub fn remove_package_from_record(package_name: &str) -> Result<(), Box<dyn Error>> {
    let mut lockfile = read_lockfile()?;

    let key_to_remove = lockfile
        .packages
        .iter()
        .find(|(_, p)| p.name == package_name)
        .map(|(k, _)| k.clone());

    if let Some(key) = key_to_remove
        && lockfile.packages.remove(&key).is_some()
    {
        lockfile.version = env!("CARGO_PKG_VERSION").to_string();
        write_lockfile(&lockfile)?;
    }

    Ok(())
}
