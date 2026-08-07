//! Command for removing unused packages (orphans).

use crate::pkg;
use anyhow::Result;
use colored::*;

/// Runs the autoremove command.
pub fn run(yes: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        println!(
            "{} Autoremoving unused packages (Dry-run)...",
            "::".bold().yellow()
        );
    } else {
        println!("{} Autoremoving unused packages...", "::".bold().blue());
    }

    pkg::autoremove::run(yes, dry_run)?;

    if !dry_run {
        println!("\n{}", "Cleanup complete.".green());
    }
    Ok(())
}
