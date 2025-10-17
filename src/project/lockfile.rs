use crate::pkg::types;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;

fn get_lockfile_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join("zoi.lock"))
}

pub fn read_zoi_lock() -> Result<types::ZoiLock> {
    let path = get_lockfile_path()?;
    if !path.exists() {
        return Ok(types::ZoiLock {
            packages: HashMap::new(),
        });
    }
    let content = fs::read_to_string(path)?;
    let lockfile = serde_json::from_str(&content)?;
    Ok(lockfile)
}

pub fn write_zoi_lock(lockfile: &types::ZoiLock) -> Result<()> {
    let path = get_lockfile_path()?;
    let content = serde_json::to_string_pretty(lockfile)?;
    fs::write(path, content)?;
    Ok(())
}
