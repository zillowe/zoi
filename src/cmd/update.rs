use crate::pkg::{config, hooks, install, local, pin, resolve, sync, transaction, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use semver::Version;
use std::collections::HashSet;
use std::fs;
use std::sync::Mutex;

pub fn run(all: bool, package_names: &[String], yes: bool) -> Result<()> {
    if all {
        if let Err(e) = run_update_all_logic(yes) {
            eprintln!("{}: {}", "Update failed".red().bold(), e);
        }
        return Ok(());
    }

    println!("{}", "--- Syncing Package Database ---".yellow().bold());
    if let Err(e) = sync::run(false, true, true) {
        eprintln!("{}: {}", "Sync failed".red().bold(), e);
        std::process::exit(1);
    }

    let mut expanded_package_names = Vec::new();
    for name in package_names {
        let request = resolve::parse_source_string(name)?;
        if request.sub_package.is_none()
            && let Ok((pkg, _, _, _, _)) = resolve::resolve_package_and_version(name)
            && pkg.sub_packages.is_some()
        {
            let installed = local::get_installed_packages()?;
            let mut installed_subs = Vec::new();
            for manifest in installed {
                if manifest.name == pkg.name
                    && let Some(sub) = manifest.sub_package
                {
                    installed_subs.push(sub);
                }
            }
            if !installed_subs.is_empty() {
                println!(
                    "'{}' is a split package. Updating all installed sub-packages: {}",
                    name,
                    installed_subs.join(", ")
                );
                for sub in installed_subs {
                    expanded_package_names.push(format!("{}:{}", name, sub));
                }
                continue;
            }
        }
        expanded_package_names.push(name.clone());
    }

    let mut failed_packages = Vec::new();

    for (i, package_name) in expanded_package_names.iter().enumerate() {
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
        println!("\n{}", "Success:".green());
    }
    Ok(())
}

fn run_update_single_logic(package_name: &str, yes: bool) -> Result<()> {
    println!("--- Updating package '{}' ---", package_name.blue().bold());

    let request = resolve::parse_source_string(package_name)?;

    let (new_pkg, new_version, _, _, registry_handle) =
        match resolve::resolve_package_and_version(package_name) {
            Ok(result) => result,
            Err(e) => {
                return Err(anyhow!(
                    "Could not resolve package '{}': {}",
                    package_name,
                    e
                ));
            }
        };

    if pin::is_pinned(package_name)? {
        println!(
            "Package '{}' is pinned. Skipping update.",
            package_name.yellow()
        );
        return Ok(());
    }

    let old_manifest = match local::is_package_installed(
        &new_pkg.name,
        request.sub_package.as_deref(),
        types::Scope::User,
    )?
    .or(local::is_package_installed(
        &new_pkg.name,
        request.sub_package.as_deref(),
        types::Scope::System,
    )?) {
        Some(m) => m,
        None => {
            return Err(anyhow!(
                "Package '{package_name}' is not installed. Use 'zoi install' instead."
            ));
        }
    };

    println!(
        "Currently installed version: {}",
        old_manifest.version.yellow()
    );
    println!("Available version: {}", new_version.green());

    if old_manifest.version == new_version {
        println!("\nPackage is already up to date.");
        return Ok(());
    }

    if !utils::ask_for_confirmation(
        &format!("Update from {} to {}?", old_manifest.version, new_version),
        yes,
    ) {
        return Ok(());
    }

    let transaction = transaction::begin()?;

    if let Some(hooks) = &new_pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreUpgrade)
    {
        return Err(anyhow!("Pre-upgrade hook failed: {}", e));
    }

    let mode = install::InstallMode::PreferPrebuilt;
    let processed_deps = Mutex::new(HashSet::new());

    match install::run_installation(
        package_name,
        mode,
        true,
        types::InstallReason::Direct,
        yes,
        false,
        &processed_deps,
        Some(old_manifest.scope),
        None,
        None,
    ) {
        Ok(new_manifest) => {
            if let Err(e) = transaction::record_operation(
                &transaction.id,
                types::TransactionOperation::Upgrade {
                    old_manifest: Box::new(old_manifest.clone()),
                    new_manifest: Box::new(new_manifest),
                },
            ) {
                eprintln!("Warning: Failed to record transaction for update: {}", e);
                transaction::delete_log(&transaction.id)?;
            } else {
                transaction::commit(&transaction.id)?;
            }

            cleanup_old_versions(
                &new_pkg.name,
                old_manifest.scope,
                &new_pkg.repo,
                registry_handle.as_deref().unwrap_or("local"),
            )?;

            if let Some(hooks) = &new_pkg.hooks
                && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostUpgrade)
            {
                return Err(anyhow!("Post-upgrade hook failed: {}", e));
            }

            println!("\n{}", "Success:".green());
            Ok(())
        }
        Err(e) => {
            eprintln!("\nError: Update failed during installation. Rolling back...");
            transaction::rollback(&transaction.id)?;
            Err(anyhow!("Update failed: {}", e))
        }
    }
}

fn run_update_all_logic(yes: bool) -> Result<()> {
    println!("{}", "--- Syncing Package Database ---".yellow().bold());
    sync::run(false, true, true)?;

    let installed_packages = local::get_installed_packages()?;
    let pinned_packages = pin::get_pinned_packages()?;
    let pinned_sources: Vec<String> = pinned_packages.into_iter().map(|p| p.source).collect();

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
        let source = format!("#{}@{}", manifest.registry_handle, manifest.repo);
        if pinned_sources.contains(&source) {
            upgrade_messages.push(format!("- {} is pinned, skipping.", manifest.name.cyan()));
            continue;
        }

        let (new_pkg, new_version, _, _, registry_handle) =
            match resolve::resolve_package_and_version(&source) {
                Ok(result) => result,
                Err(e) => {
                    upgrade_messages.push(format!(
                        "- Could not resolve package '{}': {}, skipping.",
                        manifest.name, e
                    ));
                    continue;
                }
            };

        if manifest.version != new_version {
            upgrade_messages.push(format!(
                "- {} can be upgraded from {} to {}",
                manifest.name.cyan(),
                manifest.version.yellow(),
                new_version.green()
            ));
            packages_to_upgrade.push((source.clone(), new_pkg, registry_handle, manifest));
        } else {
            upgrade_messages.push(format!("- {} is up to date.", manifest.name.cyan()));
        }
        pb.inc(1);
    }
    pb.finish_and_clear();

    for msg in upgrade_messages {
        println!("{}", msg);
    }

    if packages_to_upgrade.is_empty() {
        println!("\n{}", "Success:".green());
        return Ok(());
    }

    println!();
    if !utils::ask_for_confirmation("Do you want to upgrade these packages?", yes) {
        return Ok(());
    }

    let transaction = transaction::begin()?;
    let failed_updates = Mutex::new(Vec::new());
    let successful_upgrades = Mutex::new(Vec::new());

    packages_to_upgrade
        .par_iter()
        .for_each(|(source, new_pkg, registry_handle, old_manifest)| {
            println!(
                "\n--- Upgrading {} to {} ---",
                source.cyan(),
                new_pkg.version.as_deref().unwrap_or("N/A").green()
            );

            if let Some(hooks) = &new_pkg.hooks
                && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreUpgrade)
            {
                eprintln!(
                    "{}: Pre-upgrade hook failed for '{}': {}",
                    "Error".red().bold(),
                    source,
                    e
                );
                failed_updates.lock().unwrap().push(source.clone());
                return;
            }

            let mode = install::InstallMode::PreferPrebuilt;
            let processed_deps = Mutex::new(HashSet::new());
            match install::run_installation(
                source,
                mode,
                true,
                types::InstallReason::Direct,
                yes,
                false,
                &processed_deps,
                Some(old_manifest.scope),
                None,
                None,
            ) {
                Ok(new_manifest) => {
                    if let Err(e) = transaction::record_operation(
                        &transaction.id,
                        types::TransactionOperation::Upgrade {
                            old_manifest: Box::new(old_manifest.clone()),
                            new_manifest: Box::new(new_manifest),
                        },
                    ) {
                        eprintln!("Error: Failed to record transaction for {}: {}", source, e);
                        failed_updates.lock().unwrap().push(source.clone());
                    } else {
                        successful_upgrades.lock().unwrap().push((
                            source.clone(),
                            new_pkg.clone(),
                            registry_handle.clone(),
                            old_manifest.scope,
                        ));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to upgrade {}: {}", source, e);
                    failed_updates.lock().unwrap().push(source.clone());
                }
            }
        });

    let failed = failed_updates.into_inner().unwrap();
    if !failed.is_empty() {
        eprintln!("\nError: Some packages failed to upgrade. Rolling back all changes...");
        for pkg in &failed {
            eprintln!("  - {}", pkg);
        }
        transaction::rollback(&transaction.id)?;
        return Err(anyhow!("Update failed for some packages."));
    }

    transaction::commit(&transaction.id)?;

    println!("\n{}", "Success:".green());
    let successful_upgrades = successful_upgrades.into_inner().unwrap();
    for (source, new_pkg, registry_handle, scope) in &successful_upgrades {
        if let Err(e) = cleanup_old_versions(
            &new_pkg.name,
            *scope,
            &new_pkg.repo,
            registry_handle.as_deref().unwrap_or("local"),
        ) {
            eprintln!("Failed to clean up old versions for {}: {}", source, e);
        }

        if let Some(hooks) = &new_pkg.hooks
            && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostUpgrade)
        {
            eprintln!(
                "{}: Post-upgrade hook failed for '{}': {}",
                "Error".red().bold(),
                source,
                e
            );
        }
    }

    println!("\n{}", "Success:".green());
    Ok(())
}

fn cleanup_old_versions(
    package_name: &str,
    scope: types::Scope,
    repo: &str,
    registry_handle: &str,
) -> Result<()> {
    let config = config::read_config()?;
    let rollback_enabled = config.rollback_enabled;

    let package_dir = local::get_package_dir(scope, registry_handle, repo, package_name)?;

    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&package_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(version_str) = path.file_name().and_then(|s| s.to_str())
                && version_str != "latest"
                && let Ok(version) = Version::parse(version_str)
            {
                versions.push(version);
            }
        }
    }

    if versions.is_empty() {
        return Ok(());
    }

    versions.sort();

    let versions_to_keep = if rollback_enabled { 2 } else { 1 };

    if versions.len() > versions_to_keep {
        let num_to_delete = versions.len() - versions_to_keep;
        let versions_to_delete = &versions[..num_to_delete];

        println!("Cleaning up old versions...");
        for version in versions_to_delete {
            let version_dir_to_delete = package_dir.join(version.to_string());
            println!(" - Removing {}", version_dir_to_delete.display());
            if version_dir_to_delete.exists() {
                fs::remove_dir_all(version_dir_to_delete)?;
            }
        }
    }

    Ok(())
}
