use crate::pkg::config;
use crate::pkg::types::{InstallManifest, Scope};
use crate::pkg::utils;
use anyhow::Result;
#[cfg(windows)]
use junction;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn get_store_base_dir(scope: Scope) -> Result<PathBuf> {
    match scope {
        Scope::User => {
            let home_dir = home::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not find home directory."))?;
            Ok(home_dir.join(".zoi").join("pkgs").join("store"))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\store"))
            } else {
                Ok(PathBuf::from("/var/lib/zoi/pkgs/store"))
            }
        }
        Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("store"))
        }
    }
}

pub fn get_package_dir(
    scope: Scope,
    registry_handle: &str,
    repo_path: &str,
    package_name: &str,
) -> Result<PathBuf> {
    let base_dir = get_store_base_dir(scope)?;
    let package_id = utils::generate_package_id(registry_handle, repo_path, package_name);
    let package_dir_name = utils::get_package_dir_name(&package_id, package_name);
    Ok(base_dir.join(package_dir_name))
}

pub fn get_package_version_dir(
    scope: Scope,
    registry_handle: &str,
    repo_path: &str,
    package_name: &str,
    version: &str,
) -> Result<PathBuf> {
    let package_dir = get_package_dir(scope, registry_handle, repo_path, package_name)?;
    Ok(package_dir.join(version))
}

fn get_db_root() -> Result<PathBuf> {
    let home_dir =
        home::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

pub fn get_installed_packages() -> Result<Vec<InstallManifest>> {
    let mut installed = Vec::new();
    for scope in [Scope::User, Scope::System, Scope::Project] {
        let store_root = get_store_base_dir(scope)?;
        if !store_root.exists() {
            continue;
        }
        for entry in fs::read_dir(store_root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let latest_path = path.join("latest");
            if (latest_path.is_symlink() || latest_path.is_dir())
                && let Ok(entries) = fs::read_dir(&latest_path)
            {
                for entry in entries.filter_map(Result::ok) {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with("manifest") && file_name.ends_with(".yaml") {
                        let manifest_path = entry.path();
                        if manifest_path.exists() {
                            let content = fs::read_to_string(manifest_path)?;
                            let manifest: InstallManifest = serde_yaml::from_str(&content)?;
                            installed.push(manifest);
                        }
                    }
                }
            }
        }
    }

    installed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(installed)
}

#[derive(Debug)]
pub struct InstalledPackage {
    pub name: String,
    pub sub_package: Option<String>,
    pub version: String,
    pub repo: String,
    pub package_type: super::types::PackageType,
}

pub fn get_installed_packages_with_type() -> Result<Vec<InstalledPackage>> {
    let manifests = get_installed_packages()?;
    Ok(manifests
        .into_iter()
        .map(|m| InstalledPackage {
            name: m.name,
            sub_package: m.sub_package,
            version: m.version,
            repo: m.repo,
            package_type: m.package_type,
        })
        .collect())
}

pub fn is_package_installed(
    package_name: &str,
    sub_package_name: Option<&str>,
    scope: Scope,
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
            if parts.len() == 2 && parts[1] == package_name && parts[0].len() == 32 {
                let latest_path = path.join("latest");
                if (latest_path.is_symlink() || latest_path.is_dir())
                    && let Ok(entries) = fs::read_dir(&latest_path)
                {
                    for entry in entries.filter_map(Result::ok) {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        if file_name.starts_with("manifest") && file_name.ends_with(".yaml") {
                            let manifest_path = entry.path();
                            if manifest_path.exists() {
                                let content = fs::read_to_string(manifest_path)?;
                                let manifest: InstallManifest = serde_yaml::from_str(&content)?;
                                if manifest.name == package_name
                                    && manifest.sub_package.as_deref() == sub_package_name
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

pub fn get_packages_from_repos(repos: &[String]) -> Result<Vec<super::types::Package>> {
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
        for entry in WalkDir::new(repo_path).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_dir() {
                continue;
            }

            let pkg_name = entry.file_name().to_string_lossy();
            let pkg_file_path = entry.path().join(format!("{}.pkg.lua", pkg_name));

            if pkg_file_path.is_file() {
                let mut pkg: super::types::Package = crate::pkg::lua::parser::parse_lua_package(
                    pkg_file_path.to_str().unwrap(),
                    None,
                    true,
                )?;

                if let Ok(repo_subpath) = entry.path().strip_prefix(&db_root) {
                    pkg.repo = repo_subpath.to_string_lossy().to_string();
                }

                available.push(pkg);
            }
        }
    }

    available.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(available)
}

pub fn get_all_available_packages() -> Result<Vec<super::types::Package>> {
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
            .map(|repo| format!("{}/{}", handle, repo))
            .collect();
        get_packages_from_repos(&repos_with_handle)
    } else {
        Ok(Vec::new())
    }
}

pub fn add_dependent(package_dir: &Path, dependent_id: &str) -> Result<()> {
    let dependents_dir = package_dir.join("dependents");
    fs::create_dir_all(&dependents_dir)?;
    let dependent_file = dependents_dir.join(hex::encode(dependent_id));
    fs::write(dependent_file, "")?;
    Ok(())
}

pub fn remove_dependent(package_dir: &Path, dependent_id: &str) -> Result<()> {
    let dependents_dir = package_dir.join("dependents");
    if dependents_dir.exists() {
        let dependent_file = dependents_dir.join(hex::encode(dependent_id));
        if dependent_file.exists() {
            fs::remove_file(dependent_file)?;
        }
    }
    Ok(())
}

pub fn get_dependents(package_dir: &Path) -> Result<Vec<String>> {
    let dependents_dir = package_dir.join("dependents");
    let mut dependents = Vec::new();
    if dependents_dir.exists() {
        for entry in fs::read_dir(dependents_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && let Some(file_name) = path.file_name().and_then(|s| s.to_str())
                && let Ok(decoded) = hex::decode(file_name)
                && let Ok(dependent_id) = String::from_utf8(decoded)
            {
                dependents.push(dependent_id);
            }
        }
    }
    Ok(dependents)
}

pub fn write_manifest(manifest: &InstallManifest) -> Result<()> {
    let version_dir = get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version,
    )?;
    fs::create_dir_all(&version_dir)?;

    let manifest_filename = if let Some(sub) = &manifest.sub_package {
        format!("manifest-{}.yaml", sub)
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
        &manifest.name,
    )?;
    let latest_symlink_path = package_dir.join("latest");
    if latest_symlink_path.exists() || latest_symlink_path.is_symlink() {
        if latest_symlink_path.is_dir() {
            fs::remove_dir_all(&latest_symlink_path)?;
        } else {
            fs::remove_file(&latest_symlink_path)?;
        }
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&version_dir, &latest_symlink_path)?;
    #[cfg(windows)]
    {
        junction::create(&version_dir, &latest_symlink_path)?;
    }

    Ok(())
}
