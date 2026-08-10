//! Command for removing unused packages (orphans).

use anyhow::Result;
use colored::Colorize;

use crate::pkg;

/// Runs the autoremove command.
///
/// # Errors
///
/// Returns an error if the autoremove operation fails.
///
/// # Panics
///
/// This function does not explicitly panic.
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
