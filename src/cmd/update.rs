use crate::pkg::{install, local, pin, resolve, sync, types};
use crate::utils;
use colored::*;

pub fn run(package_name: &str) {
    let result = if package_name == "all" {
        run_update_all_logic()
    } else {
        run_update_single_logic(package_name)
    };

    if let Err(e) = result {
        eprintln!("{}: {}", "Update failed".red().bold(), e);
    }
}

fn run_update_single_logic(package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if pin::is_pinned(package_name)? {
        println!(
            "Package '{}' is pinned. Skipping update.",
            package_name.yellow()
        );
        return Ok(());
    }

    let manifest = local::is_package_installed(package_name)?.ok_or(format!(
        "Package '{package_name}' is not installed. Use 'zoi install' instead."
    ))?;

    println!("Currently installed version: {}", manifest.version.yellow());

    println!("\n{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run(false)?;

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

    let mode = if let Some(updater_method) = &new_pkg.updater {
        install::InstallMode::Updater(updater_method.clone())
    } else {
        install::InstallMode::PreferBinary
    };

    install::run_installation(
        &resolved_source.path,
        mode,
        true,
        types::InstallReason::Direct,
    )?;

    println!("\n{}", "Update complete.".green());
    Ok(())
}

fn run_update_all_logic() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run(false)?;

    let installed_packages = local::get_installed_packages()?;
    let pinned_packages = pin::get_pinned_packages()?;
    let pinned_names: Vec<String> = pinned_packages.into_iter().map(|p| p.name).collect();

    let mut packages_to_upgrade = Vec::new();

    println!("\n{}", "--- Checking for Upgrades ---".yellow().bold());
    for manifest in installed_packages {
        if pinned_names.contains(&manifest.name) {
            println!("- {} is pinned, skipping.", manifest.name.cyan());
            continue;
        }

        let resolved_source = match resolve::resolve_source(&manifest.name) {
            Ok(source) => source,
            Err(_) => {
                println!(
                    "- Could not resolve source for {}, skipping.",
                    manifest.name.red()
                );
                continue;
            }
        };

        let content = std::fs::read_to_string(&resolved_source.path)?;
        let new_pkg: crate::pkg::types::Package = serde_yaml::from_str(&content)?;

        if manifest.version != new_pkg.version {
            println!(
                "- {} can be upgraded from {} to {}",
                manifest.name.cyan(),
                manifest.version.yellow(),
                new_pkg.version.green()
            );
            packages_to_upgrade.push((
                manifest.name.clone(),
                new_pkg.version.clone(),
                resolved_source.path.clone(),
                new_pkg.updater.clone(),
            ));
        } else {
            println!("- {} is up to date.", manifest.name.cyan());
        }
    }

    if packages_to_upgrade.is_empty() {
        println!("\n{}", "All packages are up to date.".green());
        return Ok(());
    }

    println!();
    if !utils::ask_for_confirmation("Do you want to upgrade these packages?") {
        return Ok(());
    }

    for (name, version, path, updater) in packages_to_upgrade {
        println!("\n--- Upgrading {} to {} ---", name.cyan(), version.green());
        let mode = if let Some(updater_method) = updater {
            install::InstallMode::Updater(updater_method)
        } else {
            install::InstallMode::PreferBinary
        };
        install::run_installation(&path, mode, true, types::InstallReason::Direct)?;
    }

    println!("\n{}", "Upgrade complete.".green());
    Ok(())
}
