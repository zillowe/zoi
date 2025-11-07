use crate::cmd::utils as cmd_utils;
use crate::pkg::{config, hooks, install, local, pin, resolve, transaction, types};
use anyhow::{Result, anyhow};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use semver::Version;
use std::fs;
use std::sync::Mutex;

pub fn run(all: bool, package_names: &[String], yes: bool) -> Result<()> {
    if all {
        if let Err(e) = run_update_all_logic(yes) {
            eprintln!("{}: {}", "Update failed".red().bold(), e);
        }
        return Ok(());
    }

    let expanded_package_names = cmd_utils::expand_split_packages(package_names, "Updating")?;

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
        match resolve::resolve_package_and_version(package_name, true) {
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

    let download_size = new_pkg.archive_size.unwrap_or(0);
    let old_installed_size = old_manifest.installed_size.unwrap_or(0);
    let new_installed_size = new_pkg.installed_size.unwrap_or(0);
    let installed_size_diff = new_installed_size as i64 - old_installed_size as i64;

    println!();
    println!(
        "Total Download Size: {}",
        crate::utils::format_bytes(download_size)
    );
    println!(
        "Net Upgrade Size:    {}",
        crate::utils::format_size_diff(installed_size_diff)
    );
    println!();

    if !crate::utils::ask_for_confirmation(
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

    let (graph, _) = match install::resolver::resolve_dependency_graph(
        &[package_name.to_string()],
        Some(old_manifest.scope),
        true,
        yes,
        false,
        None,
        true,
    ) {
        Ok(res) => res,
        Err(e) => {
            return Err(anyhow!(
                "Failed to resolve dependency graph for update: {}",
                e
            ));
        }
    };

    let install_plan = match install::plan::create_install_plan(&graph.nodes) {
        Ok(plan) => plan,
        Err(e) => return Err(anyhow!("Failed to create install plan for update: {}", e)),
    };

    let mut new_manifest_option: Option<types::InstallManifest> = None;

    for (id, node) in &graph.nodes {
        if let Some(action) = install_plan.get(id) {
            match install::installer::install_node(node, action, None, None, yes) {
                Ok(m) => {
                    if m.name == new_pkg.name {
                        new_manifest_option = Some(m);
                    }
                }
                Err(e) => {
                    eprintln!("\nError: Update failed during installation. Rolling back...");
                    transaction::rollback(&transaction.id)?;
                    return Err(anyhow!("Update failed: {}", e));
                }
            }
        }
    }

    if let Some(new_manifest) = new_manifest_option {
        if let Err(e) = transaction::record_operation(
            &transaction.id,
            types::TransactionOperation::Upgrade {
                old_manifest: Box::new(old_manifest.clone()),
                new_manifest: Box::new(new_manifest.clone()),
            },
        ) {
            eprintln!("Warning: Failed to record transaction for update: {}", e);
            transaction::delete_log(&transaction.id)?;
        } else {
            transaction::commit(&transaction.id)?;
        }

        if let Some(backup_files) = &old_manifest.backup {
            println!("Restoring configuration files...");
            let old_version_dir = match local::get_package_version_dir(
                old_manifest.scope,
                &old_manifest.registry_handle,
                &old_manifest.repo,
                &old_manifest.name,
                &old_manifest.version,
            ) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!(
                        "Warning: could not get old version dir to restore backups: {}",
                        e
                    );
                    return Ok(());
                }
            };
            let new_version_dir = match local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version,
            ) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!(
                        "Warning: could not get new version dir to restore backups: {}",
                        e
                    );
                    return Ok(());
                }
            };

            for backup_file_rel in backup_files {
                let old_path = old_version_dir.join(backup_file_rel);
                let new_path = new_version_dir.join(backup_file_rel);

                if old_path.exists() {
                    if new_path.exists() {
                        let zoinew_path = new_path.with_extension(format!(
                            "{}.zoinew",
                            new_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                        ));
                        println!(
                            "Configuration file '{}' exists in new version. Saving as .zoinew",
                            new_path.display()
                        );
                        if let Err(e) = fs::rename(&new_path, &zoinew_path) {
                            eprintln!("Warning: failed to rename to .zoinew: {}", e);
                            continue;
                        }
                    }
                    if let Some(p) = new_path.parent()
                        && let Err(e) = fs::create_dir_all(p)
                    {
                        eprintln!(
                            "Warning: failed to create parent dir for backup restoration: {}",
                            e
                        );
                        continue;
                    }
                    if let Err(e) = fs::rename(&old_path, &new_path) {
                        eprintln!("Warning: failed to restore backup file: {}", e);
                    }
                }
            }
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
    } else {
        eprintln!("\nError: Update failed to produce a new manifest. Rolling back...");
        transaction::rollback(&transaction.id)?;
        Err(anyhow!("Update failed: could not get new manifest"))
    }
}

fn run_update_all_logic(yes: bool) -> Result<()> {
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
            match resolve::resolve_package_and_version(&source, true) {
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
        println!("\nAll packages are up to date.");
        return Ok(());
    }

    let total_download_size: u64 = packages_to_upgrade
        .iter()
        .map(|(_, pkg, _, _)| pkg.archive_size.unwrap_or(0))
        .sum();

    let total_installed_size_diff: i64 = packages_to_upgrade
        .iter()
        .map(|(_, new_pkg, _, old_manifest)| {
            let old_size = old_manifest.installed_size.unwrap_or(0) as i64;
            let new_size = new_pkg.installed_size.unwrap_or(0) as i64;
            new_size - old_size
        })
        .sum();

    println!();
    println!(
        "Total Download Size: {}",
        crate::utils::format_bytes(total_download_size)
    );
    println!(
        "Net Upgrade Size:    {}",
        crate::utils::format_size_diff(total_installed_size_diff)
    );

    println!();
    if !crate::utils::ask_for_confirmation("Do you want to upgrade these packages?", yes) {
        return Ok(());
    }

    let transaction = transaction::begin()?;
    let failed_updates = Mutex::new(Vec::new());
    let successful_upgrades = Mutex::new(Vec::new());

    packages_to_upgrade
        .par_iter()
        .for_each(|(source, new_pkg, _registry_handle, old_manifest)| {
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

            let (graph, _) = match install::resolver::resolve_dependency_graph(
                &[source.to_string()],
                Some(old_manifest.scope),
                true,
                yes,
                false,
                None,
                true,
            ) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("Error resolving dependency graph for update: {}", e);
                    failed_updates.lock().unwrap().push(source.clone());
                    return;
                }
            };

            let install_plan = match install::plan::create_install_plan(&graph.nodes) {
                Ok(plan) => plan,
                Err(e) => {
                    eprintln!("Error creating install plan for update: {}", e);
                    failed_updates.lock().unwrap().push(source.clone());
                    return;
                }
            };

            let mut new_manifest_option: Option<types::InstallManifest> = None;

            for (id, node) in &graph.nodes {
                if let Some(action) = install_plan.get(id) {
                    match install::installer::install_node(node, action, None, None, yes) {
                        Ok(m) => {
                            if m.name == new_pkg.name {
                                new_manifest_option = Some(m);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to upgrade {}: {}", source, e);
                            failed_updates.lock().unwrap().push(source.clone());
                            return;
                        }
                    }
                }
            }

            if let Some(new_manifest) = new_manifest_option {
                if let Err(e) = transaction::record_operation(
                    &transaction.id,
                    types::TransactionOperation::Upgrade {
                        old_manifest: Box::new(old_manifest.clone()),
                        new_manifest: Box::new(new_manifest.clone()),
                    },
                ) {
                    eprintln!("Error: Failed to record transaction for {}: {}", source, e);
                    failed_updates.lock().unwrap().push(source.clone());
                } else {
                    successful_upgrades.lock().unwrap().push((
                        old_manifest.clone(),
                        new_manifest.clone(),
                        new_pkg.clone(),
                    ));
                }
            } else {
                eprintln!("Failed to get new manifest for {}", source);
                failed_updates.lock().unwrap().push(source.clone());
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
    for (old_manifest, new_manifest, new_pkg) in &successful_upgrades {
        if let Some(backup_files) = &old_manifest.backup {
            println!(
                "Restoring configuration for {}...",
                old_manifest.name.cyan()
            );
            let old_version_dir = match local::get_package_version_dir(
                old_manifest.scope,
                &old_manifest.registry_handle,
                &old_manifest.repo,
                &old_manifest.name,
                &old_manifest.version,
            ) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!(
                        "Warning: could not get old version dir to restore backups: {}",
                        e
                    );
                    continue;
                }
            };
            let new_version_dir = match local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version,
            ) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!(
                        "Warning: could not get new version dir to restore backups: {}",
                        e
                    );
                    continue;
                }
            };

            for backup_file_rel in backup_files {
                let old_path = old_version_dir.join(backup_file_rel);
                let new_path = new_version_dir.join(backup_file_rel);

                if old_path.exists() {
                    if new_path.exists() {
                        let zoinew_path = new_path.with_extension(format!(
                            "{}.zoinew",
                            new_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                        ));
                        println!(
                            "Configuration file '{}' exists in new version. Saving as .zoinew",
                            new_path.display()
                        );
                        if let Err(e) = fs::rename(&new_path, &zoinew_path) {
                            eprintln!("Warning: failed to rename to .zoinew: {}", e);
                            continue;
                        }
                    }
                    if let Some(p) = new_path.parent()
                        && let Err(e) = fs::create_dir_all(p)
                    {
                        eprintln!(
                            "Warning: failed to create parent dir for backup restoration: {}",
                            e
                        );
                        continue;
                    }
                    if let Err(e) = fs::rename(&old_path, &new_path) {
                        eprintln!("Warning: failed to restore backup file: {}", e);
                    }
                }
            }
        }

        if let Err(e) = cleanup_old_versions(
            &new_manifest.name,
            new_manifest.scope,
            &new_manifest.repo,
            &new_manifest.registry_handle,
        ) {
            eprintln!(
                "Failed to clean up old versions for {}: {}",
                new_manifest.name, e
            );
        }

        if let Some(hooks) = &new_pkg.hooks
            && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostUpgrade)
        {
            eprintln!(
                "{}: Post-upgrade hook failed for '{}': {}",
                "Error".red().bold(),
                new_manifest.name,
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
