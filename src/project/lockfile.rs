use crate::pkg::types;
use anyhow::{Result, anyhow};
use std::fs;

fn get_lockfile_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

pub fn read_zoi_lock() -> Result<types::ZoiLock> {
    let path = get_lockfile_path()?;
    if !path.exists() {
        return Ok(types::ZoiLock {
            version: "1".to_string(),
            ..Default::default()
        });
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(types::ZoiLock {
            version: "1".to_string(),
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

pub fn write_zoi_lock(lockfile: &types::ZoiLock) -> Result<()> {
    let path = get_lockfile_path()?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}
