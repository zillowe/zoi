//! Implementation of the `cache` command for managing the local package cache.

use std::fs;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::pkg::cache;

/// Adds files to the local archive cache.
///
/// # Errors
///
/// Returns an error if the archive cache root cannot be determined or if
/// copying files fails.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn add(files: &[PathBuf]) -> Result<()> {
    let archive_cache_root = cache::get_archive_cache_root()?;
    fs::create_dir_all(&archive_cache_root)?;

    for file in files {
        if !file.exists() {
            eprintln!(
                "{}: File not found: {}",
                "Error".red().bold(),
                file.display()
            );
            continue;
        }
        if !file.is_file() {
            eprintln!(
                "{}: Not a file: {}",
                "Error".red().bold(),
                file.display()
            );
            continue;
        }

        let filename = file
            .file_name()
            .ok_or_else(|| anyhow!("Invalid filename"))?;
        let dest_path = archive_cache_root.join(filename);

        println!("Adding {} to cache...", filename.to_string_lossy().cyan());
        fs::copy(file, &dest_path)?;
    }

    Ok(())
}

/// Clears the entire Zoi cache.
///
/// # Errors
///
/// Returns an error if the cache clearing operation fails.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn clear(dry_run: bool) -> Result<()> {
    if dry_run {
        println!("{} Cleaning cache (Dry-run)...", "::".bold().yellow());
    } else {
        println!("{} Cleaning cache...", "::".bold().blue());
    }
    crate::pkg::cache::clear(dry_run)?;
    if !dry_run {
        println!("{}", "Cache cleaned successfully.".green());
    }
    Ok(())
}

/// Lists files in the local archive cache.
///
/// # Errors
///
/// Returns an error if the archive cache root cannot be determined or if
/// reading the directory fails.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn list() -> Result<()> {
    let archive_cache_root = cache::get_archive_cache_root()?;
    if !archive_cache_root.exists() {
        println!("Cache is empty.");
        return Ok(());
    }

    println!("{} Archives in local cache:", "::".bold().blue());
    let mut count = 0;
    for entry in fs::read_dir(archive_cache_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path
                .file_name()
                .ok_or_else(|| {
                    let p = path.display();
                    anyhow!("Path from read_dir has no file name: {p}")
                })?
                .to_string_lossy();
            let size = fs::metadata(&path)?.len();
            println!(
                "  - {:<40} ({})",
                filename.cyan(),
                crate::pkg::utils::format_bytes(size)
            );
            count += 1;
        }
    }

    if count == 0 {
        println!("No archives found in cache.");
    } else {
        println!(
            "
Total: {count} archives"
        );
    }

    Ok(())
}

/// Adds a new cache mirror URL.
///
/// # Errors
///
/// Returns an error if the mirror cannot be added to the configuration.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn add_mirror(url: &str) -> Result<()> {
    crate::pkg::config::add_cache_mirror(url)?;
    println!("Added cache mirror '{}'.", url.cyan());
    Ok(())
}

/// Removes a cache mirror URL.
///
/// # Errors
///
/// Returns an error if the mirror cannot be removed from the configuration.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn remove_mirror(url: &str) -> Result<()> {
    crate::pkg::config::remove_cache_mirror(url)?;
    println!("Removed cache mirror '{}'.", url.cyan());
    Ok(())
}

/// Lists all configured cache mirror URLs.
///
/// # Errors
///
/// Returns an error if the configuration cannot be read.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn list_mirrors() -> Result<()> {
    let config = crate::pkg::config::read_config()?;
    if config.cache_mirrors.is_empty() {
        println!("No cache mirrors configured.");
        return Ok(());
    }

    println!("{} Configured cache mirrors:", "::".bold().blue());
    for mirror in &config.cache_mirrors {
        println!("  - {}", mirror.cyan());
    }
    Ok(())
}
