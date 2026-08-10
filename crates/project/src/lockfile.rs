use std::fs;

use anyhow::{Result, anyhow};
use zoi_core::types;

/// Returns the path to the project's lockfile (`zoi.lock`).
fn get_lockfile_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

/// Reads and parses a lockfile from the specified path.
fn read_lockfile_from(
    path: &std::path::Path
) -> Result<Option<types::ZoiLockV2>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&content).map(Some).map_err(|e| {
        anyhow!(
            "Failed to parse {}. It might be corrupted or in an old format. \
             Error: {}",
            path.display(),
            e
        )
    })
}

/// Checks if the lockfile is compatible with the current platform.
fn is_lockfile_compatible(lockfile: &types::ZoiLockV2) -> bool {
    let current_platform = zoi_core::utils::get_platform().unwrap_or_default();
    if lockfile.installed_packages.is_empty() {
        return true;
    }
    lockfile.installed_packages.values().all(|pkg| {
        pkg.platform.is_empty()
            || pkg.platform == current_platform
            || zoi_core::utils::is_platform_compatible(
                &current_platform,
                std::slice::from_ref(&pkg.platform)
            )
    })
}

/// Reads the project's `zoi.lock` file, falling back to platform-specific
/// lockfiles if necessary.
///
/// # Errors
///
/// Returns an error if there is an issue reading or parsing the lockfile.
pub fn read_zoi_lock() -> Result<types::ZoiLockV2> {
    let path = get_lockfile_path()?;

    if let Some(lockfile) = read_lockfile_from(&path)? {
        if is_lockfile_compatible(&lockfile) {
            return Ok(lockfile);
        }

        let platform = zoi_core::utils::get_platform().unwrap_or_default();
        let platform_path = path.with_file_name(format!("zoi.{platform}.lock"));
        if let Some(platform_lock) = read_lockfile_from(&platform_path)? {
            return Ok(platform_lock);
        }

        eprintln!(
            "Warning: zoi.lock has packages targeting a different platform \
             and no zoi.{platform}.lock was found, falling back to \
             unconstrained resolution"
        );
    }

    Ok(types::ZoiLockV2 {
        version: "2".to_string(),
        ..Default::default()
    })
}

/// Writes the project's lockfile to disk, updating hashes for the package store
/// and registries.
///
/// # Errors
///
/// Returns an error if there is an issue writing the lockfile to disk.
pub fn write_zoi_lock(lockfile: &mut types::ZoiLockV2) -> Result<()> {
    if zoi_core::frozen::is_frozen() {
        return Ok(());
    }
    let path = get_lockfile_path()?;

    if let Ok(store_dir) =
        zoi_core::utils::get_store_base_dir(types::Scope::Project)
    {
        lockfile.packages_hash = Some(format!(
            "sha512-{}",
            zoi_core::hash::calculate_dir_hash(&store_dir).unwrap_or_default()
        ));
    }

    let db_dir = std::env::current_dir()?
        .join(".zoi")
        .join("pkgs")
        .join("db");

    if db_dir.exists() {
        lockfile.registries_hash = Some(format!(
            "sha512-{}",
            zoi_core::hash::calculate_dir_hash(&db_dir).unwrap_or_default()
        ));
    }

    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

/// Represents a package in a frozen lockfile state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenLockPackage {
    /// The package source string (e.g. `name@version`).
    pub source: String,
    /// The specific revision or commit.
    pub revision: String,
    /// Whether this was a direct project dependency.
    pub direct: bool,
    /// List of enabled build/install options.
    pub chosen_options: Vec<String>,
    /// List of enabled optional features.
    pub chosen_optionals: Vec<String>,
    /// Dependencies for this frozen package.
    pub dependencies: Option<types::DependenciesV2>,
    /// Optional Git SHA if applicable.
    pub git_sha: Option<String>
}

/// Returns a list of all packages currently recorded in the lockfile.
pub fn locked_packages(lockfile: &types::ZoiLockV2) -> Vec<FrozenLockPackage> {
    let mut packages = Vec::new();

    for (key, detail) in &lockfile.installed_packages {
        packages.push(FrozenLockPackage {
            source: format!("{}@{}", key.trim(), detail.version),
            revision: detail.revision.clone(),
            direct: detail.why == "direct",
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
            dependencies: detail.dependencies.clone(),
            git_sha: None
        });
    }

    packages.sort_by(|a, b| a.source.cmp(&b.source));
    packages
}

/// Returns a list of all package source strings from the lockfile.
pub fn sources_from_lock(lockfile: &types::ZoiLockV2) -> Vec<String> {
    locked_packages(lockfile)
        .into_iter()
        .map(|entry| entry.source)
        .collect()
}
