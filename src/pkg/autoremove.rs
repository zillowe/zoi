use crate::pkg::{dependencies, local, types::InstallReason};
use crate::utils;
use colored::*;
use std::error::Error;

pub fn run(yes: bool) -> Result<(), Box<dyn Error>> {
    println!("Checking for unused dependencies...");
    let all_installed = local::get_installed_packages()?;
    let mut packages_to_remove: Vec<String> = Vec::new();

    for package in &all_installed {
        if package.reason != InstallReason::Dependency {
            continue;
        }

        let pkg_id = format!("zoi:{}", package.name);
        let dependents = dependencies::get_dependents(&pkg_id)?;

        if dependents.is_empty() {
            packages_to_remove.push(package.name.clone());
        }
    }

    if packages_to_remove.is_empty() {
        println!("{}", "No unused dependencies to remove.".green());
        return Ok(());
    }

    println!("\nThe following packages will be REMOVED:");
    for pkg_name in &packages_to_remove {
        println!("    - {}", pkg_name.yellow());
    }

    if !utils::ask_for_confirmation("\nDo you want to continue?", yes) {
        println!("Operation aborted.");
        return Ok(());
    }

    for pkg_name in &packages_to_remove {
        println!("\n--- Removing {} ---", pkg_name.bold());
        if let Err(e) = crate::pkg::uninstall::run(pkg_name) {
            eprintln!("{} Failed to remove {}: {}", "Error:".red(), pkg_name, e);
        }
    }

    Ok(())
}
