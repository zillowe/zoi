use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run(dry_run: bool) -> Result<()> {
    if dry_run {
        println!("{}", "--- Cleaning Cache (Dry-run) ---".yellow().bold());
    } else {
        println!("{}", "--- Cleaning Cache ---".yellow().bold());
    }
    pkg::cache::clear(dry_run)?;
    pkg::cache::clear_archives(dry_run)?;
    if !dry_run {
        println!("{}", "Cache cleaned successfully.".green());
    }
    Ok(())
}
