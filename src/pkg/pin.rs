use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PinnedPackage {
    pub source: String,
    pub version: String,
}

fn get_pinned_json_path() -> Result<PathBuf, io::Error> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find home directory"))?;
    let zoi_dir = home_dir.join(".zoi");
    if !zoi_dir.exists() {
        fs::create_dir_all(&zoi_dir)?;
    }
    Ok(zoi_dir.join("pinned.json"))
}

pub fn get_pinned_packages() -> Result<Vec<PinnedPackage>, io::Error> {
    let path = get_pinned_json_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let packages: Vec<PinnedPackage> =
        serde_json::from_str(&contents).unwrap_or_else(|_| Vec::new());
    Ok(packages)
}

pub fn write_pinned_packages(packages: &[PinnedPackage]) -> Result<(), io::Error> {
    let path = get_pinned_json_path()?;
    let mut file = File::create(path)?;
    let contents = serde_json::to_string_pretty(packages)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}

pub fn get_pinned_version(source: &str) -> Result<Option<String>, io::Error> {
    let pinned_packages = get_pinned_packages()?;
    Ok(pinned_packages
        .iter()
        .find(|p| p.source == source)
        .map(|p| p.version.clone()))
}

pub fn is_pinned(source: &str) -> Result<bool, io::Error> {
    let pinned_packages = get_pinned_packages()?;
    Ok(pinned_packages.iter().any(|p| p.source == source))
}
