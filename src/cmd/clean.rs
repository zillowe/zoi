use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run() -> Result<()> {
    println!("{}", "--- Cleaning Cache ---".yellow().bold());
    pkg::cache::clear()?;
    pkg::cache::clear_archives()?;
    println!("{}", "Cache cleaned successfully.".green());
    Ok(())
}
