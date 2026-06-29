use anyhow::{Result, anyhow};
use std::fs;
use zoi_core::types;

fn get_lockfile_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

pub fn read_zoi_lock() -> Result<types::ZoiLockV2> {
    let path = get_lockfile_path()?;
    if !path.exists() {
        return Ok(types::ZoiLockV2 {
            version: "2".to_string(),
            ..Default::default()
        });
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(types::ZoiLockV2 {
            version: "2".to_string(),
            ..Default::default()
        });
    }

    serde_json::from_str(&content).map_err(|e| {
        anyhow!(
            "Failed to parse zoi.lock. It might be corrupted or in an old format. Error: {}",
            e
        )
    })
}

pub fn write_zoi_lock(lockfile: &types::ZoiLockV2) -> Result<()> {
    let path = get_lockfile_path()?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenLockPackage {
    pub source: String,
    pub revision: String,
    pub direct: bool,
    pub chosen_options: Vec<String>,
    pub chosen_optionals: Vec<String>,
    pub dependencies: Option<types::DependenciesV2>,
    pub git_sha: Option<String>,
}

pub fn locked_packages(lockfile: &types::ZoiLockV2) -> Vec<FrozenLockPackage> {
    let mut packages = Vec::new();

    for (key, detail) in &lockfile.installed_packages {
        packages.push(FrozenLockPackage {
            source: format!("{}@{}", key, detail.version),
            revision: detail.revision.clone(),
            direct: detail.why == "direct",
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
            dependencies: detail.dependencies.clone(),
            git_sha: None,
        });
    }

    packages.sort_by(|a, b| a.source.cmp(&b.source));
    packages
}

pub fn sources_from_lock(lockfile: &types::ZoiLockV2) -> Vec<String> {
    locked_packages(lockfile)
        .into_iter()
        .map(|entry| entry.source)
        .collect()
}
