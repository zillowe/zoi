use crate::pkg::cache;
use anyhow::{Result, anyhow};
use colored::*;
use std::fs;
use std::path::PathBuf;

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
            eprintln!("{}: Not a file: {}", "Error".red().bold(), file.display());
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

pub fn clear(dry_run: bool) -> Result<()> {
    crate::cmd::clean::run(dry_run)
}

pub fn list() -> Result<()> {
    let archive_cache_root = cache::get_archive_cache_root()?;
    if !archive_cache_root.exists() {
        println!("Cache is empty.");
        return Ok(());
    }

    println!("{}", "--- Archives in local cache ---".yellow().bold());
    let mut count = 0;
    for entry in fs::read_dir(archive_cache_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_string_lossy();
            let size = fs::metadata(&path)?.len();
            println!(
                "  - {:<40} ({})",
                filename.cyan(),
                crate::utils::format_bytes(size)
            );
            count += 1;
        }
    }

    if count == 0 {
        println!("No archives found in cache.");
    } else {
        println!(
            "
Total: {} archives",
            count
        );
    }

    Ok(())
}
