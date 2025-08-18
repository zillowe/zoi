use crate::pkg::types::{Package, RecordedPackage};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn get_recorder_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let path = home_dir.join(".zoi").join("pkgs").join("zoi.pkgs.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

pub fn read_recorded_packages() -> Result<Vec<RecordedPackage>, Box<dyn Error>> {
    let path = get_recorder_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    if content.is_empty() {
        return Ok(Vec::new());
    }
    let packages: Vec<RecordedPackage> = serde_json::from_str(&content)?;
    Ok(packages)
}

pub fn write_recorded_packages(packages: &[RecordedPackage]) -> Result<(), Box<dyn Error>> {
    let path = get_recorder_path()?;
    let content = serde_json::to_string_pretty(packages)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn record_package(pkg: &Package) -> Result<(), Box<dyn Error>> {
    let version = pkg.version.as_ref().ok_or("Package version not resolved")?;
    let new_package = RecordedPackage {
        name: pkg.name.clone(),
        repo: pkg.repo.clone(),
        version: version.clone(),
    };

    let mut packages = read_recorded_packages().unwrap_or_else(|_| Vec::new());

    if let Some(existing_pkg) = packages.iter_mut().find(|p| p.name == new_package.name) {
        existing_pkg.version = new_package.version;
        existing_pkg.repo = new_package.repo;
    } else {
        packages.push(new_package);
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));

    write_recorded_packages(&packages)
}
