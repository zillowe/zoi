use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run(yes: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        println!(
            "{}",
            "--- Autoremoving Unused Packages (Dry-run) ---".yellow()
        );
    } else {
        println!("{}", "--- Autoremoving Unused Packages ---".yellow());
    }

    pkg::autoremove::run(yes, dry_run)?;

    if !dry_run {
        println!("\n{}", "Cleanup complete.".green());
    }
    Ok(())
}
