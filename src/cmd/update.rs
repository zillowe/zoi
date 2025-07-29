use crate::pkg::{install, local, pin, resolve, sync, types};
use crate::utils;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};

pub fn run(package_name: &str, yes: bool) {
    let result = if package_name == "all" {
        run_update_all_logic(yes)
    } else {
        run_update_single_logic(package_name, yes)
    };

    if let Err(e) = result {
        eprintln!("{}: {}", "Update failed".red().bold(), e);
    }
}

fn run_update_single_logic(
    package_name: &str,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if pin::is_pinned(package_name)? {
        println!(
            "Package '{}' is pinned. Skipping update.",
            package_name.yellow()
        );
        return Ok(());
    }

    let manifest =
        local::is_package_installed(package_name, types::Scope::User)?
            .or(local::is_package_installed(package_name, types::Scope::System)?)
            .ok_or(format!(
                "Package '{package_name}' is not installed. Use 'zoi install' instead."
            ))?;

    println!("Currently installed version: {}", manifest.version.yellow());

    println!("\n{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run(false)?;

    let resolved_source = resolve::resolve_source(package_name)?;
    let content = std::fs::read_to_string(&resolved_source.path)?;
    let new_pkg: crate::pkg::types::Package = serde_yaml::from_str(&content)?;

    println!("Available version: {}", new_pkg.version.as_deref().unwrap_or("N/A").green());

    if manifest.version == new_pkg.version.as_deref().unwrap_or_default() {
        println!("\nPackage is already up to date.");
        return Ok(());
    }

    if !utils::ask_for_confirmation(
        &format!(
            "Update from {} to {}?",
            manifest.version,
            new_pkg.version.as_deref().unwrap_or("N/A")
        ),
        yes,
    ) {
        return Ok(());
    }

    let mode = if let Some(updater_method) = &new_pkg.updater {
        install::InstallMode::Updater(updater_method.clone())
    } else {
        install::InstallMode::PreferBinary
    };

    install::run_installation(
        resolved_source.path.to_str().unwrap(),
        mode,
        true,
        types::InstallReason::Direct,
        yes,
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
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")?,
    );
    pb.set_message("Checking for available updates...");

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
        install::run_installation(
            path.to_str().unwrap(),
            mode,
            true,
            types::InstallReason::Direct,
            yes,
        )?;
    }

    println!("\n{}", "Upgrade complete.".green());
    Ok(())
}
