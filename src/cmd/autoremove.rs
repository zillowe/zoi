use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run(yes: bool) -> Result<()> {
    println!("{}", "--- Autoremoving Unused Packages ---".yellow());

    pkg::autoremove::run(yes)?;

    println!("\n{}", "Cleanup complete.".green());
    Ok(())
}
