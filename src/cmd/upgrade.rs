use crate::pkg;
use anyhow::Result;
use colored::*;

pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>,
) -> Result<()> {
    println!("{} Upgrading Zoi...", "::".bold().blue());

    match pkg::upgrade::run(branch, status, number, force, tag, custom_branch) {
        Ok(()) => {}
        Err(e) if e.to_string() == "already_on_latest" => {}
        Err(e) => return Err(e),
    }
    Ok(())
}
