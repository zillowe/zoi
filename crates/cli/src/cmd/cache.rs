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
/// As root this also lists the system-wide archive cache that
/// `zoi system distro build` and other sysroot installs use.
///
/// # Errors
///
/// Returns an error if the archive cache root cannot be determined or if
/// reading the directory fails.
pub fn list() -> Result<()> {
    let dirs = cache::archive_cache_dirs()?;
    let mut total = 0;
    for (i, dir) in dirs.iter().enumerate() {
        if i > 0 {
            println!();
        }
        total += list_dir(dir, dirs.len() > 1)?;
    }
    if dirs.len() > 1 && total > 0 {
        println!("\nTotal: {total} archives");
    }
    Ok(())
}

/// Lists the archives in one cache directory and returns how many there are.
fn list_dir(dir: &std::path::Path, show_path: bool) -> Result<usize> {
    if show_path {
        println!(
            "{} Archives in {}:",
            "::".bold().blue(),
            dir.display().to_string().cyan()
        );
    } else {
        println!("{} Archives in local cache:", "::".bold().blue());
    }
    if !dir.exists() {
        println!("Cache is empty.");
        return Ok(0);
    }

    let mut count = 0;
    let mut entries: Vec<_> =
        fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        let size = fs::metadata(&path)?.len();
        let is_partial = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("part"));
        let note = if is_partial {
            " (incomplete download)".dimmed().to_string()
        } else {
            String::new()
        };
        println!(
            "  - {:<40} ({}){note}",
            filename.cyan(),
            crate::pkg::utils::format_bytes(size)
        );
        count += 1;
    }

    if count == 0 {
        println!("No archives found in cache.");
    } else if !show_path {
        println!("\nTotal: {count} archives");
    } else {
        println!("{count} archives");
    }
    Ok(count)
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
