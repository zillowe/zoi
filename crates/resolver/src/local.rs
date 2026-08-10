//! Local package management and store interaction.
//!
//! This module provides functions for interacting with the local package store,
//! listing installed packages, and managing package manifests and dependencies.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;
use zoi_core::types::{self, InstallManifest, Scope};
use zoi_core::{config, utils};

use crate::resolve::{PackageRequest, get_db_root};

/// Returns the base directory of the package store for a given scope.
///
/// # Errors
///
/// Returns an error if the store base directory cannot be determined.
pub fn get_store_base_dir(scope: Scope) -> Result<PathBuf> {
    utils::get_store_base_dir(scope)
}

/// Returns the directory for a specific package in the store.
///
/// # Errors
///
/// Returns an error if the package directory path cannot be constructed.
pub fn get_package_dir(
    scope: Scope,
    registry_handle: &str,
    repo_path: &str,
    package_name: &str
) -> Result<PathBuf> {
    let base_dir = get_store_base_dir(scope)?;
    let package_id =
        utils::generate_package_id(registry_handle, repo_path, package_name);
    let package_dir_name =
        utils::get_package_dir_name(&package_id, package_name);
    Ok(base_dir.join(package_dir_name))
}

/// Returns the directory for a specific version of a package in the store.
///
/// # Errors
///
/// Returns an error if the package version directory path cannot be
/// constructed.
pub fn get_package_version_dir(
    scope: Scope,
    registry_handle: &str,
    repo_path: &str,
    package_name: &str,
    version: &str
) -> Result<PathBuf> {
    let package_dir =
        get_package_dir(scope, registry_handle, repo_path, package_name)?;
    Ok(package_dir.join(version))
}

use rayon::prelude::*;

/// Returns a list of all installed packages across all scopes.
///
/// # Errors
///
/// Returns an error if the store cannot be accessed or manifests cannot be
/// read.
pub fn get_installed_packages() -> Result<Vec<InstallManifest>> {
    let scopes = [Scope::User, Scope::System, Scope::Project];

    let installed: Vec<InstallManifest> = scopes
        .into_par_iter()
        .map(|scope| {
            let mut manifests = Vec::new();
            if let Ok(store_root) = get_store_base_dir(scope)
                && store_root.exists()
                && let Ok(entries) = fs::read_dir(store_root)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let latest_path = path.join("latest");
                    if (latest_path.is_symlink() || latest_path.is_dir())
                        && let Ok(sub_entries) = fs::read_dir(&latest_path)
                    {
                        for sub_entry in sub_entries.flatten() {
                            let file_name = sub_entry
                                .file_name()
                                .to_string_lossy()
                                .to_string();
                            if file_name.starts_with("manifest")
                                && std::path::Path::new(&file_name)
                                    .extension()
                                    .is_some_and(|ext| {
                                        ext.eq_ignore_ascii_case("yaml")
                                    })
                            {
                                let manifest_path = sub_entry.path();
                                if manifest_path.exists()
                                    && let Ok(content) =
                                        fs::read_to_string(manifest_path)
                                    && let Ok(manifest) =
                                        serde_yaml::from_str::<InstallManifest>(
                                            &content
                                        )
                                {
                                    manifests.push(manifest);
                                }
                            }
                        }
                    }
                }
            }
            manifests
        })
        .flatten()
        .collect();

    let mut sorted_installed = installed;
    sorted_installed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sorted_installed)
}

/// Represents an installed package with basic metadata.
#[derive(Debug)]
pub struct InstalledPackage {
    /// The name of the package.
    pub name: String,
    /// The sub-package name, if any.
    pub sub_package: Option<String>,
    /// The installed version.
    pub version: String,
    /// The repository the package was installed from.
    pub repo: String,
    /// The type of package.
    pub package_type: zoi_core::types::PackageType
}

/// Returns a list of all installed packages with their basic metadata.
///
/// # Errors
///
/// Returns an error if installed packages cannot be retrieved.
pub fn get_installed_packages_with_type() -> Result<Vec<InstalledPackage>> {
    let manifests = get_installed_packages()?;
    Ok(manifests
        .into_iter()
        .map(|m| InstalledPackage {
            name: m.name,
            sub_package: m.sub_package,
            version: m.version,
            repo: m.repo,
            package_type: m.package_type
        })
        .collect())
}

/// Checks if a package is installed in a given scope and returns its manifest
/// if found.
///
/// # Errors
///
/// Returns an error if the store cannot be accessed or manifests cannot be
/// read.
pub fn is_package_installed(
    package_name: &str,
    sub_package_name: Option<&str>,
    scope: Scope
) -> Result<Option<InstallManifest>> {
    let store_root = get_store_base_dir(scope)?;
    if !store_root.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(store_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let parts: Vec<&str> = file_name.splitn(2, '-').collect();
            if parts.len() == 2
                && parts.get(1) == Some(&package_name)
                && parts.first().is_some_and(|p| p.len() == 32)
            {
                let latest_path = path.join("latest");
                if (latest_path.is_symlink() || latest_path.is_dir())
                    && let Ok(entries) = fs::read_dir(&latest_path)
                {
                    for entry in entries.filter_map(Result::ok) {
                        let file_name =
                            entry.file_name().to_string_lossy().to_string();
                        if file_name.starts_with("manifest")
                            && std::path::Path::new(&file_name)
                                .extension()
                                .is_some_and(|ext| {
                                    ext.eq_ignore_ascii_case("yaml")
                                })
                        {
                            let manifest_path = entry.path();
                            if manifest_path.exists() {
                                let content =
                                    fs::read_to_string(manifest_path)?;
                                let manifest: InstallManifest =
                                    serde_yaml::from_str(&content)?;
                                if manifest.name == package_name
                                    && manifest.sub_package.as_deref()
                                        == sub_package_name
                                {
                                    return Ok(Some(manifest));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Returns all installed manifests in a specific scope.
///
/// # Errors
///
/// Returns an error if the store cannot be accessed or manifests cannot be
/// read.
pub fn get_installed_manifests_in_scope(
    scope: Scope
) -> Result<Vec<InstallManifest>> {
    let store_root = get_store_base_dir(scope)?;
    if !store_root.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();
    for entry in fs::read_dir(store_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let latest_path = path.join("latest");
        if !(latest_path.is_symlink() || latest_path.is_dir()) {
            continue;
        }

        let Ok(entries) = fs::read_dir(&latest_path) else {
            continue;
        };

        for entry in entries.filter_map(Result::ok) {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("manifest")
                || !std::path::Path::new(&file_name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml"))
            {
                continue;
            }

            let manifest_path = entry.path();
            if !manifest_path.exists() {
                continue;
            }

            let content = fs::read_to_string(manifest_path)?;
            let manifest: InstallManifest = serde_yaml::from_str(&content)?;
            manifests.push(manifest);
        }
    }

    Ok(manifests)
}

/// Finds installed manifests matching a package request in a specific scope.
///
/// # Errors
///
/// Returns an error if installed manifests cannot be retrieved.
pub fn find_installed_manifests_matching(
    request: &PackageRequest,
    scope: Scope
) -> Result<Vec<InstallManifest>> {
    let manifests = get_installed_manifests_in_scope(scope)?;
    Ok(manifests
        .into_iter()
        .filter(|manifest| {
            manifest.name == request.name
                && manifest.sub_package == request.sub_package
                && request
                    .handle
                    .as_ref()
                    .is_none_or(|handle| manifest.registry_handle == *handle)
                && request
                    .repo
                    .as_ref()
                    .is_none_or(|repo| manifest.repo == *repo)
                && request
                    .version_spec
                    .as_ref()
                    .is_none_or(|version| manifest.version == *version)
        })
        .collect())
}

/// Formats a package source string from its components.
pub fn package_source_string(
    registry_handle: &str,
    repo: &str,
    name: &str,
    sub_package: Option<&str>,
    version: &str
) -> String {
    let registry_handle = registry_handle.trim();
    let repo = repo.trim();
    let name = name.trim();
    let version = version.trim();

    if let Some(sub_package) = sub_package {
        format!(
            "#{}@{}/{}:{}@{}",
            registry_handle,
            repo,
            name,
            sub_package.trim(),
            version
        )
    } else {
        format!("#{registry_handle}@{repo}/{name}@{version}")
    }
}

/// Returns the source string for an installed manifest.
pub fn installed_manifest_source(manifest: &InstallManifest) -> String {
    package_source_string(
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        manifest.sub_package.as_deref(),
        &manifest.version
    )
}

/// Returns all available packages from a list of repositories.
///
/// # Errors
///
/// Returns an error if the database root cannot be determined or if package
/// files cannot be parsed.
pub fn get_packages_from_repos(
    repos: &[String]
) -> Result<Vec<zoi_core::types::Package>> {
    let db_root = get_db_root()?;
    if !db_root.exists() {
        return Err(anyhow::anyhow!(
            "Package database not found. Please run 'zoi sync' first."
        ));
    }

    let mut available = Vec::new();

    for repo_name in repos {
        let repo_path = db_root.join(repo_name);
        if !repo_path.exists() {
            continue;
        }
        for entry in WalkDir::new(repo_path).into_iter().filter_map(Result::ok)
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let pkg_name = entry.file_name().to_string_lossy();
            let pkg_file_path =
                entry.path().join(format!("{pkg_name}.pkg.lua"));

            if pkg_file_path.is_file() {
                let pkg_file_path_str =
                    pkg_file_path.to_str().ok_or_else(|| {
                        anyhow::anyhow!(
                            "Package path contains invalid UTF-8: {}",
                            pkg_file_path.display()
                        )
                    })?;
                let mut pkg: zoi_core::types::Package =
                    zoi_lua::parser::parse_lua_package(
                        pkg_file_path_str,
                        None,
                        None,
                        true
                    )?;

                if let Ok(repo_subpath) = entry.path().strip_prefix(&db_root) {
                    let mut repo_path = repo_subpath
                        .to_string_lossy()
                        .to_string()
                        .replace('\\', "/");
                    let pkg_name_suffix = format!("/{}", pkg.name);
                    if repo_path.ends_with(&pkg_name_suffix) {
                        repo_path = repo_path
                            [..repo_path.len() - pkg_name_suffix.len()]
                            .to_string();
                    } else if repo_path == pkg.name {
                        repo_path = String::new();
                    }
                    pkg.repo = repo_path;
                }

                available.push(pkg);
            }
        }
    }

    available.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(available)
}

/// Returns all available packages in the default registry.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read or available packages
/// cannot be retrieved.
pub fn get_all_available_packages() -> Result<Vec<zoi_core::types::Package>> {
    let config = config::read_config()?;
    if let Some(handle) = config
        .default_registry
        .as_ref()
        .map(|r| &r.handle)
        .filter(|h| !h.is_empty())
    {
        let repos_with_handle: Vec<String> = config
            .repos
            .iter()
            .map(|repo| format!("{handle}/{repo}"))
            .collect();
        get_packages_from_repos(&repos_with_handle)
    } else {
        Ok(Vec::new())
    }
}

/// Adds a dependent ID to a package's dependents list.
///
/// # Errors
///
/// Returns an error if the dependent file cannot be written.
pub fn add_dependent(package_dir: &Path, dependent_id: &str) -> Result<()> {
    let dependents_dir = package_dir.join("dependents");
    fs::create_dir_all(&dependents_dir)?;
    let dependent_file = dependents_dir.join(hex::encode(dependent_id));
    fs::write(dependent_file, "")?;
    Ok(())
}

/// Removes a dependent ID from a package's dependents list.
///
/// # Errors
///
/// Returns an error if the dependent file cannot be removed.
pub fn remove_dependent(package_dir: &Path, dependent_id: &str) -> Result<()> {
    let dependents_dir = package_dir.join("dependents");
    if dependents_dir.exists() {
        let dependent_file = dependents_dir.join(hex::encode(dependent_id));
        if dependent_file.exists() {
            fs::remove_file(dependent_file)?;
        } else if let Some(pos) = dependent_id.rfind('@') {
            let legacy_id = &dependent_id[..pos];
            let legacy_file = dependents_dir.join(hex::encode(legacy_id));
            if legacy_file.exists() {
                fs::remove_file(legacy_file)?;
            }
        }
    }
    Ok(())
}

/// Returns a list of all dependent IDs for a package.
///
/// # Errors
///
/// Returns an error if the dependents directory cannot be read.
pub fn get_dependents(package_dir: &Path) -> Result<Vec<String>> {
    let dependents_dir = package_dir.join("dependents");
    let mut dependents = Vec::new();
    if dependents_dir.exists() {
        for entry in fs::read_dir(dependents_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && let Some(file_name) =
                    path.file_name().and_then(|s| s.to_str())
                && let Ok(decoded) = hex::decode(file_name)
                && let Ok(dependent_id) = String::from_utf8(decoded)
            {
                dependents.push(dependent_id);
            }
        }
    }
    Ok(dependents)
}

/// Writes an installation manifest to the store and updates the 'latest'
/// symlink.
///
/// # Errors
///
/// Returns an error if the manifest cannot be serialized or written.
pub fn write_manifest(manifest: &InstallManifest) -> Result<()> {
    let version_dir = get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )?;
    fs::create_dir_all(&version_dir)?;

    let manifest_filename = if let Some(sub) = &manifest.sub_package {
        format!("manifest-{sub}.yaml")
    } else {
        "manifest.yaml".to_string()
    };
    let manifest_path = version_dir.join(manifest_filename);

    let content = serde_yaml::to_string(&manifest)?;
    fs::write(manifest_path, content)?;

    let package_dir = get_package_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name
    )?;
    let latest_symlink_path = package_dir.join("latest");
    zoi_core::utils::symlink_dir(&version_dir, &latest_symlink_path)?;

    Ok(())
}

/// Returns the path where the package source file should be stored.
///
/// # Errors
///
/// Returns an error if the source path cannot be constructed.
pub fn get_package_source_path(manifest: &InstallManifest) -> Result<PathBuf> {
    let version_dir = get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )?;
    Ok(version_dir.join("package.pkg.lua"))
}

/// Persists the package source file to the store.
///
/// # Errors
///
/// Returns an error if the source file cannot be copied.
pub fn persist_package_source(
    manifest: &InstallManifest,
    source_path: &Path
) -> Result<()> {
    let stored_source_path = get_package_source_path(manifest)?;
    if let Some(parent) = stored_source_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source_path, stored_source_path)?;
    Ok(())
}

/// Updates the installation reason in a package's manifest.
///
/// # Errors
///
/// Returns an error if the manifest cannot be updated.
pub fn update_manifest_reason(
    manifest: &InstallManifest,
    new_reason: types::InstallReason
) -> Result<()> {
    let mut updated_manifest = manifest.clone();
    updated_manifest.reason = new_reason;
    write_manifest(&updated_manifest)?;
    Ok(())
}
