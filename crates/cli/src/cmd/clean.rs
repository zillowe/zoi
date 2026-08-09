//! Command for cleaning the package cache.

use anyhow::Result;
use colored::Colorize;

use crate::pkg;

/// Runs the clean command.
///
/// # Errors
///
/// Returns an error if the cache clearing operation fails.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn run(dry_run: bool,) -> Result<(),> {
    if dry_run {
        println!("{} Cleaning cache (Dry-run)...", "::".bold().yellow());
    } else {
        println!("{} Cleaning cache...", "::".bold().blue());
    }
    pkg::cache::clear(dry_run,)?;
    pkg::cache::clear_archives(dry_run,)?;
    if !dry_run {
        println!("{}", "Cache cleaned successfully.".green());
    }
    Ok((),)
}
