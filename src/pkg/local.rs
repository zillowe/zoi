use crate::pkg::types::InstallManifest;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn get_store_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("store"))
}

fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

pub fn get_installed_packages() -> Result<Vec<InstallManifest>, Box<dyn Error>> {
    let store_root = get_store_root()?;
    if !store_root.exists() {
        return Ok(Vec::new());
    }

    let mut installed = Vec::new();
    for entry in fs::read_dir(store_root)? {
        let entry = entry?;
        let manifest_path = entry.path().join("manifest.yaml");
        if manifest_path.exists() {
            let content = fs::read_to_string(manifest_path)?;
            let manifest: InstallManifest = serde_yaml::from_str(&content)?;
            installed.push(manifest);
        }
    }
    installed.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(installed)
}

pub fn is_package_installed(package_name: &str) -> Result<Option<InstallManifest>, Box<dyn Error>> {
    let manifest_path = get_store_root()?.join(package_name).join("manifest.yaml");
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

    let mut available = Vec::new();
    for entry in WalkDir::new(db_root).into_iter().filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().ends_with(".pkg.yaml") {
            let content = fs::read_to_string(entry.path())?;
            let pkg: super::types::Package = serde_yaml::from_str(&content)?;
            available.push(pkg);
        }
    }
    available.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(available)
}

pub fn write_manifest(manifest: &InstallManifest) -> Result<(), Box<dyn Error>> {
    let store_dir = get_store_root()?.join(&manifest.name);
    fs::create_dir_all(&store_dir)?;
    let manifest_path = store_dir.join("manifest.yaml");
    let content = serde_yaml::to_string(&manifest)?;
    fs::write(manifest_path, content)?;
    Ok(())
}
