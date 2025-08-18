use crate::pkg::config;
use crate::pkg::types::{InstallManifest, Package, Scope};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn get_store_root(scope: Scope) -> Result<PathBuf, Box<dyn Error>> {
    match scope {
        Scope::User => {
            let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
            Ok(home_dir.join(".zoi").join("pkgs").join("store"))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\store"))
            } else {
                Ok(PathBuf::from("/var/lib/zoi/pkgs/store"))
            }
        }
    }
}

fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

pub fn get_installed_packages() -> Result<Vec<InstallManifest>, Box<dyn Error>> {
    let mut installed = Vec::new();
    let user_store_root = get_store_root(Scope::User)?;
    if user_store_root.exists() {
        for entry in fs::read_dir(user_store_root)? {
            let entry = entry?;
            let manifest_path = entry.path().join("manifest.yaml");
            if manifest_path.exists() {
                let content = fs::read_to_string(manifest_path)?;
                let manifest: InstallManifest = serde_yaml::from_str(&content)?;
                installed.push(manifest);
            }
        }
    }

    let system_store_root = get_store_root(Scope::System)?;
    if system_store_root.exists() {
        for entry in fs::read_dir(system_store_root)? {
            let entry = entry?;
            let manifest_path = entry.path().join("manifest.yaml");
            if manifest_path.exists() {
                let content = fs::read_to_string(manifest_path)?;
                let manifest: InstallManifest = serde_yaml::from_str(&content)?;
                installed.push(manifest);
            }
        }
    }

    installed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(installed)
}

#[derive(Debug)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub package_type: super::types::PackageType,
}

pub fn get_installed_packages_with_type() -> Result<Vec<InstalledPackage>, Box<dyn Error>> {
    let manifests = get_installed_packages()?;
    let mut packages = Vec::new();

    for manifest in manifests {
        let db_root = get_db_root()?;
        if !db_root.exists() {
            continue;
        }

        let mut pkg_file: Option<PathBuf> = None;

        if !manifest.repo.is_empty() {
            let path = db_root
                .join(&manifest.repo)
                .join(format!("{}.pkg.yaml", manifest.name));
            if path.exists() {
                pkg_file = Some(path);
            }
        }

        if pkg_file.is_none() {
            for entry in WalkDir::new(&db_root).into_iter().filter_map(Result::ok) {
                if entry.file_name().to_string_lossy() == format!("{}.pkg.yaml", manifest.name) {
                    pkg_file = Some(entry.path().to_path_buf());
                    break;
                }
            }
        }

        if let Some(path) = pkg_file {
            let content = fs::read_to_string(&path)?;
            let pkg: Package = serde_yaml::from_str(&content)?;

            let mut repo_field = manifest.repo.clone();
            if repo_field.is_empty()
                && let Some(parent_dir) = path.parent()
                && let Ok(repo_subpath) = parent_dir.strip_prefix(&db_root)
            {
                repo_field = repo_subpath.to_string_lossy().to_string();
            }

            packages.push(InstalledPackage {
                name: manifest.name,
                version: manifest.version,
                repo: repo_field,
                package_type: pkg.package_type,
            });
        }
    }
    Ok(packages)
}

pub fn is_package_installed(
    package_name: &str,
    scope: Scope,
) -> Result<Option<InstallManifest>, Box<dyn Error>> {
    let manifest_path = get_store_root(scope)?
        .join(package_name)
        .join("manifest.yaml");
    if manifest_path.exists() {
        let content = fs::read_to_string(manifest_path)?;
        let manifest: InstallManifest = serde_yaml::from_str(&content)?;
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

pub fn get_all_available_packages() -> Result<Vec<super::types::Package>, Box<dyn Error>> {
    let db_root = get_db_root()?;
    if !db_root.exists() {
        return Err("Package database not found. Please run 'zoi sync' first.".into());
    }

    let config = config::read_config()?;
    let mut available = Vec::new();

    for repo_name in config.repos {
        let repo_path = db_root.join(&repo_name);
        if !repo_path.exists() {
            continue;
        }
        for entry in WalkDir::new(repo_path).into_iter().filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().ends_with(".pkg.yaml") {
                let content = fs::read_to_string(entry.path())?;
                let mut pkg: super::types::Package = serde_yaml::from_str(&content)?;

                if let Some(parent_dir) = entry.path().parent()
                    && let Ok(repo_subpath) = parent_dir.strip_prefix(&db_root)
                {
                    pkg.repo = repo_subpath.to_string_lossy().to_string();
                }

                available.push(pkg);
            }
        }
    }

    available.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(available)
}

pub fn write_manifest(manifest: &InstallManifest) -> Result<(), Box<dyn Error>> {
    let store_dir = get_store_root(manifest.scope)?.join(&manifest.name);
    fs::create_dir_all(&store_dir)?;
    let manifest_path = store_dir.join("manifest.yaml");
    let content = serde_yaml::to_string(&manifest)?;
    fs::write(manifest_path, content)?;
    Ok(())
}

pub fn remove_manifest(package_name: &str, scope: Scope) -> Result<(), Box<dyn Error>> {
    let manifest_path = get_store_root(scope)?
        .join(package_name)
        .join("manifest.yaml");
    if manifest_path.exists() {
        fs::remove_file(manifest_path)?;
    }
    Ok(())
}
