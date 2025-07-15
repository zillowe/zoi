use crate::pkg::{local, resolve, sync};
use crate::utils;
use colored::*;

pub fn run(package_name: &str) {
    if let Err(e) = run_update_logic(package_name) {
        eprintln!("{}: {}", "Update failed".red().bold(), e);
    }
}

fn run_update_logic(package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = local::is_package_installed(package_name)?.ok_or(format!(
        "Package '{}' is not installed. Use 'zoi install' instead.",
        package_name
    ))?;

    println!("Currently installed version: {}", manifest.version.yellow());

    println!("\n{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run()?;

    let resolved_source = resolve::resolve_source(package_name)?;
    let content = std::fs::read_to_string(&resolved_source.path)?;
    let new_pkg: crate::pkg::types::Package = serde_yaml::from_str(&content)?;

    println!("Available version: {}", new_pkg.version.green());

    if manifest.version == new_pkg.version {
        println!("\nPackage is already up to date.");
        return Ok(());
    }

    if !utils::ask_for_confirmation(&format!(
        "Update from {} to {}?",
        manifest.version, new_pkg.version
    )) {
        return Ok(());
    }

    super::install::run(package_name, true);

    println!("\n{}", "Update complete.".green());
    Ok(())
}
