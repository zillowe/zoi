use crate::pkg::{install, local, pin, resolve, sync, types};
use crate::utils;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;

pub fn run(package_names: &[String], yes: bool) {
    if package_names.len() == 1 && package_names[0] == "all" {
        if let Err(e) = run_update_all_logic(yes) {
            eprintln!("{}: {}", "Update failed".red().bold(), e);
        }
        return;
    }

    println!("{}", "--- Syncing Package Database ---".yellow().bold());
    if let Err(e) = sync::run(false) {
        eprintln!("{}: {}", "Sync failed".red().bold(), e);
        std::process::exit(1);
    }

    let mut failed_packages = Vec::new();

    for (i, package_name) in package_names.iter().enumerate() {
        if i > 0 {
            println!();
        }
        if let Err(e) = run_update_single_logic(package_name, yes) {
            eprintln!(
                "{}: Failed to update '{}': {}",
                "Error".red().bold(),
                package_name,
                e
            );
            failed_packages.push(package_name.clone());
        }
    }

    if !failed_packages.is_empty() {
        eprintln!(
            "\n{}: The following packages failed to update:",
            "Error".red().bold()
        );
        for pkg in &failed_packages {
            eprintln!("  - {}", pkg);
        }
        std::process::exit(1);
    } else if !package_names.is_empty() {
        println!(
            "\n{}",
            "Update process finished for all specified packages.".green()
        );
    }
}

fn run_update_single_logic(
    package_name: &str,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Updating package '{}' ---", package_name.blue().bold());
    let lower_package_name = package_name.to_lowercase();

    if pin::is_pinned(&lower_package_name)? {
        println!(
            "Package '{}' is pinned. Skipping update.",
            package_name.yellow()
        );
        return Ok(());
    }

    let manifest = local::is_package_installed(&lower_package_name, types::Scope::User)?
        .or(local::is_package_installed(
            &lower_package_name,
            types::Scope::System,
        )?) 
        .ok_or(format!(
            "Package '{package_name}' is not installed. Use 'zoi install' instead."
        ))?;

    println!("Currently installed version: {}", manifest.version.yellow());

    let (new_pkg, new_version) = resolve::resolve_package_and_version(&lower_package_name)?;

    println!("Available version: {}", new_version.green());

    if manifest.version == new_version {
        println!("\nPackage is already up to date.");
        return Ok(());
    }

    if !utils::ask_for_confirmation(
        &format!("Update from {} to {}?", manifest.version, new_version),
        yes,
    ) {
        return Ok(());
    }

    let mode = if let Some(updater_method) = &new_pkg.updater {
        install::InstallMode::Updater(updater_method.clone())
    } else {
        install::InstallMode::PreferBinary
    };

    let mut processed_deps = HashSet::new();
    install::run_installation(
        &lower_package_name,
        mode,
        true,
        types::InstallReason::Direct,
        yes,
        &mut processed_deps,
    )?;

    println!("\n{}", "Update complete.".green());
    Ok(())
}


fn run_update_all_logic(yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run(false)?;

    let installed_packages = local::get_installed_packages()?;
    let pinned_packages = pin::get_pinned_packages()?;
    let pinned_names: Vec<String> = pinned_packages.into_iter().map(|p| p.name).collect();

    let mut packages_to_upgrade = Vec::new();
    let mut upgrade_messages = Vec::new();

    println!("\n{}", "--- Checking for Upgrades ---".yellow().bold());
    let pb = ProgressBar::new(installed_packages.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({msg})",
            )?
            .progress_chars("#>-"),
    );
    pb.set_message("Checking packages...");

    for manifest in installed_packages {
        if pinned_names.contains(&manifest.name) {
            upgrade_messages.push(format!("- {} is pinned, skipping.", manifest.name.cyan()));
            continue;
        }

        let resolved_source = match resolve::resolve_source(&manifest.name) {
            Ok(source) => source,
            Err(_) => {
                upgrade_messages.push(format!(
                    "- Could not resolve source for {}, skipping.",
                    manifest.name.red()
                ));
                continue;
            }
        };

        let content = std::fs::read_to_string(&resolved_source.path)?;
        let new_pkg: crate::pkg::types::Package = serde_yaml::from_str(&content)?;

        if manifest.version != new_pkg.version.as_deref().unwrap_or_default() {
            upgrade_messages.push(format!(
                "- {} can be upgraded from {} to {}",
                manifest.name.cyan(),
                manifest.version.yellow(),
                new_pkg.version.as_deref().unwrap_or("N/A").green()
            ));
            packages_to_upgrade.push((
                manifest.name.clone(),
                new_pkg.version.clone(),
                resolved_source.path.clone(),
                new_pkg.updater.clone(),
            ));
        } else {
            upgrade_messages.push(format!("- {} is up to date.", manifest.name.cyan()));
        }
    }
    pb.finish_and_clear();

    for msg in upgrade_messages {
        println!("{}", msg);
    }

    if packages_to_upgrade.is_empty() {
        println!("\n{}", "All packages are up to date.".green());
        return Ok(());
    }

    println!();
    if !utils::ask_for_confirmation("Do you want to upgrade these packages?", yes) {
        return Ok(());
    }

    for (name, version, path, updater) in packages_to_upgrade {
        println!(
            "\n--- Upgrading {} to {} ---",
            name.cyan(),
            version.as_deref().unwrap_or("N/A").green()
        );
        let mode = if let Some(updater_method) = updater {
            install::InstallMode::Updater(updater_method)
        } else {
            install::InstallMode::PreferBinary
        };
        let mut processed_deps = HashSet::new();
        install::run_installation(
            path.to_str().unwrap(),
            mode,
            true,
            types::InstallReason::Direct,
            yes,
            &mut processed_deps,
        )?;
    }

    println!("\n{}", "Upgrade complete.".green());
    Ok(())
}
