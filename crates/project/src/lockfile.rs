//! Project lockfile persistence and package snapshot extraction.

use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde::Serialize;
use sha2::{Digest, Sha512};
use zoi_core::types;

use crate::config::ProjectConfig;

/// Returns the lockfile path in the current project directory.
///
/// # Errors
///
/// Returns an error if the current working directory cannot be determined.
fn get_lockfile_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

/// Computes the prefixed SHA-512 hash of byte content.
fn hash_bytes(content: &[u8]) -> String {
    format!("sha512-{}", hex::encode(Sha512::digest(content)))
}

/// Serializes a value and computes its prefixed SHA-512 hash.
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
fn hash_serializable<T: Serialize>(value: &T) -> Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(value)?))
}

/// Records the evaluated project manifest and its configuration sections.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or the lockfile cannot be
/// updated.
pub fn record_project_config(
    config: &ProjectConfig,
    manifest_path: &std::path::Path
) -> Result<()> {
    if zoi_core::frozen::is_frozen() {
        return Ok(());
    }

    let manifest = fs::read(manifest_path)?;
    let mut lockfile = read_zoi_lock()?;
    let manifest_hash = hash_bytes(&manifest);
    lockfile.version = "2".to_string();
    lockfile.platform = Some(zoi_core::utils::get_platform()?);
    lockfile.manifest = Some(types::LockManifestV2 {
        path: manifest_path
            .file_name()
            .unwrap_or(manifest_path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        hash: manifest_hash.clone()
    });
    lockfile.project = Some(types::LockProjectV2 {
        manifest_hash,
        tasks_hash: hash_serializable(&config.commands)?,
        environments_hash: hash_serializable(&config.environments)?,
        shell_hash: hash_serializable(&config.shell)?,
        checks_hash: hash_serializable(&config.packages)?,
        local: config.config.local
    });
    lockfile.root_requirements = config
        .pkgs_v2
        .iter()
        .map(|(source, spec)| {
            Ok(types::LockRequirementV2 {
                source: source.clone(),
                declared_by: "packages".to_string(),
                spec: serde_json::to_value(spec)?
            })
        })
        .collect::<Result<Vec<_>>>()?;
    lockfile
        .root_requirements
        .sort_by(|left, right| left.source.cmp(&right.source));
    write_zoi_lock(&mut lockfile)
}

/// Records resolved project imports in `zoi.lock`.
///
/// # Errors
///
/// Returns an error if the lockfile cannot be updated.
pub fn record_imports(
    imports: &std::collections::BTreeMap<String, types::LockImportV2>
) -> Result<()> {
    if zoi_core::frozen::is_frozen() {
        return Ok(());
    }
    let mut lockfile = read_zoi_lock()?;
    lockfile.imports = imports.clone();
    write_zoi_lock(&mut lockfile)
}

/// Reads and parses a lockfile from a specific path.
///
/// # Errors
///
/// Returns an error if an existing lockfile cannot be read or parsed.
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
    serde_json::from_str(&content).map(Some).map_err(|error| {
        anyhow!(
            "Failed to parse {}. It might be corrupted or in an old format. \
             Error: {}",
            path.display(),
            error
        )
    })
}

/// Reports whether a lockfile supports the current platform.
fn is_lockfile_compatible(lockfile: &types::ZoiLockV2) -> bool {
    let current_platform = zoi_core::utils::get_platform().unwrap_or_default();
    if let Some(platform) = &lockfile.platform
        && !platform.is_empty()
        && platform != &current_platform
        && !zoi_core::utils::is_platform_compatible(
            &current_platform,
            std::slice::from_ref(platform)
        )
    {
        return false;
    }
    if lockfile.installed_packages.is_empty() {
        return true;
    }
    lockfile.installed_packages.values().all(|package| {
        package.platform.is_empty()
            || package.platform == current_platform
            || zoi_core::utils::is_platform_compatible(
                &current_platform,
                std::slice::from_ref(&package.platform)
            )
    })
}

/// Reads and parses the project's `zoi.lock` file, falling back to
/// platform-specific lockfiles when necessary.
///
/// # Errors
///
/// Returns an error if the lockfile is incompatible, missing, or malformed.
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

        return Err(anyhow!(
            "zoi.lock targets an incompatible platform and no \
             zoi.{platform}.lock was found"
        ));
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
    /// The package source string.
    pub source: String,
    /// The specific revision or commit.
    pub revision: String,
    /// Whether this was a direct project dependency.
    pub direct: bool,
    /// List of enabled build/install options.
    pub chosen_options: Vec<String>,
    /// List of enabled optional features.
    pub chosen_optionals: Vec<String>,
    /// Declared dependencies for this frozen package.
    pub dependencies: Option<types::DependenciesV2>,
    /// Exact package identifiers selected as dependencies.
    pub resolved_dependencies: Vec<String>,
    /// Optional Git SHA if applicable.
    pub git_sha: Option<String>
}

/// Returns a list of all packages currently recorded in the lockfile.
pub fn locked_packages(lockfile: &types::ZoiLockV2) -> Vec<FrozenLockPackage> {
    let mut packages = Vec::new();

    for detail in lockfile.installed_packages.values() {
        let legacy_source = if let Some(sub_package) = &detail.sub_package {
            format!(
                "#{}@{}/{}:{}@{}",
                detail.registry,
                detail.repo,
                detail.name,
                sub_package,
                detail.version
            )
        } else {
            format!(
                "#{}@{}/{}@{}",
                detail.registry, detail.repo, detail.name, detail.version
            )
        };
        let source = detail
            .source
            .as_ref()
            .filter(|source| {
                !source.request.is_empty()
                    && (source.kind != "local"
                        || source.path.as_deref().is_none_or(str::is_empty))
            })
            .map_or_else(
                || {
                    detail
                        .source
                        .as_ref()
                        .filter(|source| source.kind == "local")
                        .and_then(|source| source.path.clone())
                        .unwrap_or(legacy_source)
                },
                |source| source.request.clone()
            );
        let git_sha = detail.git_sha.clone().or_else(|| {
            detail
                .source
                .as_ref()
                .and_then(|source| source.revision.clone())
        });
        packages.push(FrozenLockPackage {
            source,
            revision: detail.revision.clone(),
            direct: detail.why == "direct",
            chosen_options: detail.chosen_options.clone(),
            chosen_optionals: detail.chosen_optionals.clone(),
            dependencies: detail.dependencies.clone(),
            resolved_dependencies: detail.resolved_dependencies.clone(),
            git_sha
        });
    }

    packages.sort_by(|left, right| left.source.cmp(&right.source));
    packages
}

/// Returns a list of all package source strings from the lockfile.
pub fn sources_from_lock(lockfile: &types::ZoiLockV2) -> Vec<String> {
    locked_packages(lockfile)
        .into_iter()
        .map(|entry| entry.source)
        .collect()
}
