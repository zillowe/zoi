use crate::pkg::sysroot::apply_sysroot;
use anyhow::{Result, anyhow};
use std::fs;
use std::path::PathBuf;

pub fn get_cache_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(apply_sysroot(home_dir.join(".zoi").join("cache")))
}

pub fn get_archive_cache_root() -> Result<PathBuf> {
    let cache_root = get_cache_root()?;
    Ok(cache_root.join("archives"))
}

pub fn clear(dry_run: bool) -> Result<()> {
    let cache_dir = get_cache_root()?;
    if cache_dir.exists() {
        if dry_run {
            println!(
                "(Dry-run) Would remove cache directory: {}",
                cache_dir.display()
            );
        } else {
            println!("Removing cache directory: {}", cache_dir.display());
            fs::remove_dir_all(cache_dir)?;
        }
    } else {
        println!("Cache directory does not exist. Nothing to clean.");
    }
    Ok(())
}

pub fn clear_archives(dry_run: bool) -> Result<()> {
    let archive_cache_dir = get_archive_cache_root()?;
    if archive_cache_dir.exists() {
        if dry_run {
            println!(
                "(Dry-run) Would remove archive cache directory: {}",
                archive_cache_dir.display()
            );
        } else {
            println!(
                "Removing archive cache directory: {}",
                archive_cache_dir.display()
            );
            fs::remove_dir_all(archive_cache_dir)?;
        }
    } else {
        println!("Archive cache directory does not exist. Nothing to clean.");
    }
    Ok(())
}
