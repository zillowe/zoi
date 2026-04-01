use crate::cmd::utils as cmd_utils;
use crate::cmd::ux;
use crate::pkg::{config, db, hooks, install, local, pin, resolve, transaction, types};
use anyhow::{Result, anyhow};
use colored::*;
use dialoguer::{MultiSelect, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use semver::Version;
use serde_json::json;
use std::fs;
use std::sync::Mutex;

pub fn run(
    all: bool,
    package_names: &[String],
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
    interactive: bool,
) -> Result<()> {
    if all {
        return run_update_all_logic(yes, dry_run, explain, plan_json, interactive);
    }

    let expanded_package_names = cmd_utils::expand_split_packages(package_names, "Updating")?;

    let mut failed_packages = Vec::new();

    for (i, package_name) in expanded_package_names.iter().enumerate() {
        if i > 0 {
            println!();
        }
        if let Err(e) = run_update_single_logic(package_name, yes, dry_run, explain, plan_json) {
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
        return Err(anyhow!(
            "The following packages failed to update: {}",
            failed_packages.join(", ")
        ));
    } else if !package_names.is_empty() && !dry_run {
        println!("\n{}", "Success:".green());
    }
    Ok(())
}

fn run_update_single_logic(
    package_name: &str,
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
) -> Result<()> {
    println!(
        "{} Updating package '{}'...",
        "::".bold().blue(),
        package_name.cyan().bold()
    );

    let request = resolve::parse_source_string(package_name)?;

    let (new_pkg, new_version, _, _, registry_handle, _) =
        resolve::resolve_package_and_version(package_name, false, yes)?;

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
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: 1,
        });
        return Ok(());
    }

    let download_size = new_pkg.archive_size.unwrap_or(0);
    let old_installed_size = old_manifest.installed_size.unwrap_or(0);
    let new_installed_size = new_pkg.installed_size.unwrap_or(0);
    let installed_size_diff = new_installed_size as i64 - old_installed_size as i64;

    println!();
    if download_size > 0 {
        println!(
            "Total Download Size: {}",
            crate::utils::format_bytes(download_size)
        );
    }
    if installed_size_diff != 0 {
        println!(
            "Net Upgrade Size:    {}",
            crate::utils::format_size_diff(installed_size_diff)
        );
    }
    println!();

    let preflight = ux::PreflightSummary::new("Update preflight")
        .row("Package", new_pkg.name.clone())
        .row("From", old_manifest.version.clone())
        .row("To", new_version.clone())
        .row("Scope", format!("{:?}", old_manifest.scope))
        .row("Download size", crate::utils::format_bytes(download_size))
        .row(
            "Net size",
            crate::utils::format_size_diff(installed_size_diff),
        );
    ux::print_preflight(&preflight);

    if explain {
        let mut report = ux::ExplainReport::new("Update explanation");
        report = report.item(
            new_pkg.name.clone(),
            format!(
                "selected because newer version {} is available over installed {}",
                new_version, old_manifest.version
            ),
            Vec::new(),
        );
        if let Ok((old_adv, new_adv)) = advisory_counts(
            &old_manifest.registry_handle,
            &new_pkg.name,
            old_manifest.sub_package.as_deref(),
            &old_manifest.version,
            &new_version,
        ) {
            report = report.item(
                "advisories",
                format!(
                    "old={}, new={}, delta={}",
                    old_adv,
                    new_adv,
                    (new_adv as i64 - old_adv as i64)
                ),
                Vec::new(),
            );
        }
        ux::print_explain(&report);
    }

    if plan_json {
        let plan = json!({
            "dry_run": dry_run,
            "package": {
                "name": new_pkg.name,
                "sub_package": old_manifest.sub_package,
                "registry": old_manifest.registry_handle,
                "repo": old_manifest.repo,
                "scope": format!("{:?}", old_manifest.scope),
                "from_version": old_manifest.version,
                "to_version": new_version,
                "download_bytes": download_size,
                "net_size_bytes": installed_size_diff,
            }
        });
        ux::emit_plan_json_v1("update", plan)?;
    }

    if dry_run {
        println!(
            "{} Dry-run: would upgrade {} from {} to {}",
            "::".bold().yellow(),
            new_pkg.name,
            old_manifest.version,
            new_version
        );
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: 1,
        });
        return Ok(());
    }

    if !crate::utils::ask_for_confirmation(
        &format!("Update from {} to {}?", old_manifest.version, new_version),
        yes,
    ) {
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: 1,
        });
        return Ok(());
    }

    let transaction = transaction::begin()?;

    if let Some(hooks) = &new_pkg.hooks {
        hooks::run_hooks(hooks, hooks::HookType::PreUpgrade)?;
    }

    let (graph, _) = install::resolver::resolve_dependency_graph(
        &[package_name.to_string()],
        Some(old_manifest.scope),
        true,
        yes,
        false,
        None,
        false,
    )?;

    install::util::check_policy_compliance(&graph)?;
    install::util::check_for_vulnerabilities(&graph, yes)?;

    let install_plan = install::plan::create_install_plan(&graph.nodes, None, false)?;

    let stages = graph.toposort()?;
    let mut new_manifest_option: Option<types::InstallManifest> = None;

    for stage in stages {
        for pkg_id in stage {
            let node = graph
                .nodes
                .get(&pkg_id)
                .expect("Package node missing from graph during update");
            if let Some(action) = install_plan.get(&pkg_id) {
                match install::installer::install_node(node, action, None, None, yes, true) {
                    Ok(m) => {
                        if m.name == new_pkg.name {
                            new_manifest_option = Some(m);
                        }
                    }
                    Err(e) => {
                        eprintln!("\nError: Update failed during installation. Rolling back...");
                        transaction::rollback(&transaction.id)?;
                        ux::print_transaction_summary(&ux::TransactionSummary {
                            command: "update".to_string(),
                            success: 0,
                            failed: 1,
                            skipped: 0,
                        });
                        return Err(anyhow!("Update failed: {}", e));
                    }
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
            if let Ok(modified_files) = transaction::get_modified_files(&transaction.id) {
                let _ = crate::pkg::hooks::global::run_global_hooks(
                    crate::pkg::hooks::global::HookWhen::PostTransaction,
                    &modified_files,
                    "upgrade",
                );
            }
            transaction::commit(&transaction.id)?;
        }

        if let Some(backup_files) = &old_manifest.backup {
            println!("Restoring configuration files...");
            let old_version_dir = local::get_package_version_dir(
                old_manifest.scope,
                &old_manifest.registry_handle,
                &old_manifest.repo,
                &old_manifest.name,
                &old_manifest.version,
            )?;
            let new_version_dir = local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version,
            )?;

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
                    if let Some(p) = new_path.parent() {
                        fs::create_dir_all(p)?;
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

        let handle = registry_handle.as_deref().unwrap_or("local");
        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                &new_pkg,
                handle,
                Some(new_pkg.scope),
                request.sub_package.as_deref(),
                Some(&types::InstallReason::Direct),
            );
        }

        if let Some(hooks) = &new_pkg.hooks {
            hooks::run_hooks(hooks, hooks::HookType::PostUpgrade)?;
        }

        println!("\n{}", "Success:".green());
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 1,
            failed: 0,
            skipped: 0,
        });
        Ok(())
    } else {
        eprintln!("\nError: Update failed to produce a new manifest. Rolling back...");
        transaction::rollback(&transaction.id)?;
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 1,
            skipped: 0,
        });
        Err(anyhow!("Update failed: could not get new manifest"))
    }
}

fn run_update_all_logic(
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
    interactive: bool,
) -> Result<()> {
    #[derive(Clone)]
    struct UpdateCandidate {
        source: String,
        new_pkg: types::Package,
        new_version: String,
        old_manifest: types::InstallManifest,
        old_advisories: usize,
        new_advisories: usize,
    }

    let installed_packages = local::get_installed_packages()?;
    let mut pinned_sources = Vec::new();
    let mut skipped_sources = Vec::new();
    let mut up_to_date_sources = Vec::new();
    let mut packages_to_upgrade: Vec<UpdateCandidate> = Vec::new();

    println!("\n{} Checking for upgrades...", "::".bold().blue());
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
        let source = if let Some(sub) = &manifest.sub_package {
            format!(
                "#{}@{}/{}:{}",
                manifest.registry_handle, manifest.repo, manifest.name, sub
            )
        } else {
            format!(
                "#{}@{}/{}",
                manifest.registry_handle, manifest.repo, manifest.name
            )
        };

        if pin::is_pinned(&source).unwrap_or(false)
            || pin::is_pinned(&manifest.name).unwrap_or(false)
        {
            pinned_sources.push(source.clone());
            pb.inc(1);
            continue;
        }

        let (new_pkg, new_version, _, _, _registry_handle, _) =
            match resolve::resolve_package_and_version(&source, true, false) {
                Ok(result) => result,
                Err(e) => {
                    skipped_sources.push(format!("{} ({})", source, e));
                    pb.inc(1);
                    continue;
                }
            };

        if manifest.version == new_version {
            up_to_date_sources.push(source.clone());
            pb.inc(1);
            continue;
        }

        let (old_adv, new_adv) = advisory_counts(
            &manifest.registry_handle,
            &manifest.name,
            manifest.sub_package.as_deref(),
            &manifest.version,
            &new_version,
        )
        .unwrap_or((0, 0));

        packages_to_upgrade.push(UpdateCandidate {
            source,
            new_pkg,
            new_version,
            old_manifest: manifest,
            old_advisories: old_adv,
            new_advisories: new_adv,
        });
        pb.inc(1);
    }
    pb.finish_and_clear();

    if !pinned_sources.is_empty() {
        println!("\n{} Pinned (skipped)", "::".bold().blue());
        for source in &pinned_sources {
            println!("  - {}", source.yellow());
        }
    }
    if !skipped_sources.is_empty() {
        println!("\n{} Skipped (resolve failures)", "::".bold().blue());
        for source in &skipped_sources {
            println!("  - {}", source.red());
        }
    }
    if !packages_to_upgrade.is_empty() {
        println!("\n{} Upgrade candidates", "::".bold().blue());
        for candidate in &packages_to_upgrade {
            let delta = candidate.new_advisories as i64 - candidate.old_advisories as i64;
            let advisory_suffix = if delta > 0 {
                format!(" (advisories +{})", delta).red().to_string()
            } else if delta < 0 {
                format!(" (advisories {})", delta).green().to_string()
            } else {
                String::new()
            };
            println!(
                "  - {}: {} -> {}{}",
                candidate.source.cyan(),
                candidate.old_manifest.version.yellow(),
                candidate.new_version.green(),
                advisory_suffix
            );
        }
    }

    if packages_to_upgrade.is_empty() {
        println!("\nAll packages are up to date.");
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: pinned_sources.len() + skipped_sources.len() + up_to_date_sources.len(),
        });
        return Ok(());
    }

    if explain {
        let mut report = ux::ExplainReport::new("Update explanation");
        for candidate in &packages_to_upgrade {
            report = report.item(
                candidate.source.clone(),
                format!(
                    "selected because {} -> {}",
                    candidate.old_manifest.version, candidate.new_version
                ),
                Vec::new(),
            );
        }
        ux::print_explain(&report);
    }

    if interactive && !dry_run {
        let items: Vec<String> = packages_to_upgrade
            .iter()
            .map(|c| {
                format!(
                    "{}  {} -> {}",
                    c.source, c.old_manifest.version, c.new_version
                )
            })
            .collect();
        let selected = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select packages to update")
            .items(&items)
            .interact()
            .map_err(|e| anyhow!("Interactive selection failed: {}", e))?;

        if selected.is_empty() {
            println!("No packages selected.");
            ux::print_transaction_summary(&ux::TransactionSummary {
                command: "update".to_string(),
                success: 0,
                failed: 0,
                skipped: packages_to_upgrade.len()
                    + pinned_sources.len()
                    + skipped_sources.len()
                    + up_to_date_sources.len(),
            });
            return Ok(());
        }

        let selected_set: std::collections::HashSet<usize> = selected.into_iter().collect();
        let mut filtered = Vec::new();
        for (idx, candidate) in packages_to_upgrade.into_iter().enumerate() {
            if selected_set.contains(&idx) {
                filtered.push(candidate);
            }
        }
        packages_to_upgrade = filtered;
    }

    let total_download_size: u64 = packages_to_upgrade
        .iter()
        .map(|c| c.new_pkg.archive_size.unwrap_or(0))
        .sum();
    let total_installed_size_diff: i64 = packages_to_upgrade
        .iter()
        .map(|c| {
            c.new_pkg.installed_size.unwrap_or(0) as i64
                - c.old_manifest.installed_size.unwrap_or(0) as i64
        })
        .sum();

    let preflight = ux::PreflightSummary::new("Update preflight")
        .row("Candidates", packages_to_upgrade.len().to_string())
        .row("Pinned skipped", pinned_sources.len().to_string())
        .row("Other skipped", skipped_sources.len().to_string())
        .row("Up-to-date", up_to_date_sources.len().to_string())
        .row(
            "Download size",
            crate::utils::format_bytes(total_download_size),
        )
        .row(
            "Net size",
            crate::utils::format_size_diff(total_installed_size_diff),
        );
    ux::print_preflight(&preflight);

    if plan_json {
        let packages: Vec<_> = packages_to_upgrade
            .iter()
            .map(|c| {
                json!({
                    "source": c.source,
                    "name": c.new_pkg.name,
                    "sub_package": c.old_manifest.sub_package,
                    "from_version": c.old_manifest.version,
                    "to_version": c.new_version,
                    "download_bytes": c.new_pkg.archive_size.unwrap_or(0),
                    "net_size_bytes": c.new_pkg.installed_size.unwrap_or(0) as i64 - c.old_manifest.installed_size.unwrap_or(0) as i64,
                    "advisories_old": c.old_advisories,
                    "advisories_new": c.new_advisories,
                })
            })
            .collect();

        let plan = json!({
            "dry_run": dry_run,
            "interactive": interactive,
            "totals": {
                "candidates": packages_to_upgrade.len(),
                "pinned_skipped": pinned_sources.len(),
                "other_skipped": skipped_sources.len(),
                "up_to_date": up_to_date_sources.len(),
                "download_bytes": total_download_size,
                "net_size_bytes": total_installed_size_diff,
            },
            "pinned": pinned_sources,
            "skipped": skipped_sources,
            "packages": packages,
        });
        ux::emit_plan_json_v1("update", plan)?;
    }

    if dry_run {
        println!(
            "\n{} Dry-run: upgrade plan above would be executed.",
            "::".bold().yellow()
        );
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: packages_to_upgrade.len()
                + pinned_sources.len()
                + skipped_sources.len()
                + up_to_date_sources.len(),
        });
        return Ok(());
    }

    println!();
    if !crate::utils::ask_for_confirmation("Do you want to upgrade these packages?", yes) {
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: packages_to_upgrade.len()
                + pinned_sources.len()
                + skipped_sources.len()
                + up_to_date_sources.len(),
        });
        return Ok(());
    }

    let transaction = transaction::begin()?;
    let transaction_id = &transaction.id;
    let transaction_mutex = Mutex::new(());
    let failed_updates = Mutex::new(Vec::new());
    let successful_upgrades = Mutex::new(Vec::new());

    packages_to_upgrade.par_iter().for_each(|candidate| {
        println!(
            "\n{} Upgrading {} to {}...",
            "::".bold().blue(),
            candidate.source.cyan(),
            candidate.new_version.green()
        );

        if let Some(hooks) = &candidate.new_pkg.hooks
            && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreUpgrade)
        {
            eprintln!(
                "{}: Pre-upgrade hook failed for '{}': {}",
                "Error".red().bold(),
                candidate.source,
                e
            );
            failed_updates
                .lock()
                .expect("mutex poisoned")
                .push(candidate.source.clone());
            return;
        }

        let (graph, _) = match install::resolver::resolve_dependency_graph(
            std::slice::from_ref(&candidate.source),
            Some(candidate.old_manifest.scope),
            true,
            yes,
            false,
            None,
            false,
        ) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error resolving dependency graph for update: {}", e);
                failed_updates
                    .lock()
                    .expect("mutex poisoned")
                    .push(candidate.source.clone());
                return;
            }
        };

        if let Err(e) = install::util::check_policy_compliance(&graph) {
            eprintln!("Policy check failed for {}: {}", candidate.source, e);
            failed_updates
                .lock()
                .expect("mutex poisoned")
                .push(candidate.source.clone());
            return;
        }

        if let Err(e) = install::util::check_for_vulnerabilities(&graph, yes) {
            eprintln!("Security check failed for {}: {}", candidate.source, e);
            failed_updates
                .lock()
                .expect("mutex poisoned")
                .push(candidate.source.clone());
            return;
        }

        let install_plan = match install::plan::create_install_plan(&graph.nodes, None, false) {
            Ok(plan) => plan,
            Err(e) => {
                eprintln!("Error creating install plan for update: {}", e);
                failed_updates
                    .lock()
                    .expect("mutex poisoned")
                    .push(candidate.source.clone());
                return;
            }
        };

        let stages = match graph.toposort() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error sorting dependency graph for update: {}", e);
                failed_updates
                    .lock()
                    .expect("mutex poisoned")
                    .push(candidate.source.clone());
                return;
            }
        };

        let mut new_manifest_option: Option<types::InstallManifest> = None;
        for stage in stages {
            for pkg_id in stage {
                let node = graph
                    .nodes
                    .get(&pkg_id)
                    .expect("Package node missing from graph during update");
                if let Some(action) = install_plan.get(&pkg_id) {
                    match install::installer::install_node(node, action, None, None, yes, true) {
                        Ok(m) => {
                            if m.name == candidate.new_pkg.name
                                && m.sub_package == candidate.old_manifest.sub_package
                            {
                                new_manifest_option = Some(m);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to upgrade {}: {}", candidate.source, e);
                            failed_updates
                                .lock()
                                .expect("mutex poisoned")
                                .push(candidate.source.clone());
                            return;
                        }
                    }
                }
            }
        }

        if let Some(new_manifest) = new_manifest_option {
            let _lock = transaction_mutex.lock().expect("mutex poisoned");
            if let Err(e) = transaction::record_operation(
                transaction_id,
                types::TransactionOperation::Upgrade {
                    old_manifest: Box::new(candidate.old_manifest.clone()),
                    new_manifest: Box::new(new_manifest.clone()),
                },
            ) {
                eprintln!(
                    "Error: Failed to record transaction for {}: {}",
                    candidate.source, e
                );
                failed_updates
                    .lock()
                    .expect("mutex poisoned")
                    .push(candidate.source.clone());
            } else {
                successful_upgrades.lock().expect("mutex poisoned").push((
                    candidate.old_manifest.clone(),
                    new_manifest.clone(),
                    candidate.new_pkg.clone(),
                ));
            }
        } else {
            eprintln!("Failed to get new manifest for {}", candidate.source);
            failed_updates
                .lock()
                .expect("mutex poisoned")
                .push(candidate.source.clone());
        }
    });

    let failed = failed_updates.into_inner().expect("mutex poisoned");
    if !failed.is_empty() {
        eprintln!("\nError: Some packages failed to upgrade. Rolling back all changes...");
        for pkg in &failed {
            eprintln!("  - {}", pkg);
        }
        transaction::rollback(&transaction.id)?;
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: failed.len(),
            skipped: pinned_sources.len() + skipped_sources.len() + up_to_date_sources.len(),
        });
        return Err(anyhow!("Update failed for some packages."));
    }

    if let Ok(modified_files) = transaction::get_modified_files(&transaction.id) {
        let _ = crate::pkg::hooks::global::run_global_hooks(
            crate::pkg::hooks::global::HookWhen::PostTransaction,
            &modified_files,
            "upgrade",
        );
    }
    transaction::commit(&transaction.id)?;

    println!("\n{}", "Success:".green());
    let successful_upgrades = successful_upgrades.into_inner().expect("mutex poisoned");
    for (old_manifest, new_manifest, new_pkg) in &successful_upgrades {
        if let Some(backup_files) = &old_manifest.backup {
            println!(
                "Restoring configuration for {}...",
                old_manifest.name.cyan()
            );
            let old_version_dir = local::get_package_version_dir(
                old_manifest.scope,
                &old_manifest.registry_handle,
                &old_manifest.repo,
                &old_manifest.name,
                &old_manifest.version,
            )?;
            let new_version_dir = local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version,
            )?;
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
                    if let Some(p) = new_path.parent() {
                        fs::create_dir_all(p)?;
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

        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                new_pkg,
                &new_manifest.registry_handle,
                Some(new_manifest.scope),
                new_manifest.sub_package.as_deref(),
                Some(&old_manifest.reason),
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

    ux::print_transaction_summary(&ux::TransactionSummary {
        command: "update".to_string(),
        success: successful_upgrades.len(),
        failed: 0,
        skipped: pinned_sources.len() + skipped_sources.len() + up_to_date_sources.len(),
    });
    println!("\n{}", "Success:".green());
    Ok(())
}

fn advisory_counts(
    registry_handle: &str,
    package: &str,
    sub_package: Option<&str>,
    old_version: &str,
    new_version: &str,
) -> Result<(usize, usize)> {
    let advisories = db::get_advisories_for_package(registry_handle, package, sub_package)?;
    let old_ver = Version::parse(old_version)
        .map_err(|e| anyhow!("failed to parse old version '{}': {}", old_version, e))?;
    let new_ver = Version::parse(new_version)
        .map_err(|e| anyhow!("failed to parse new version '{}': {}", new_version, e))?;

    let mut old_count = 0usize;
    let mut new_count = 0usize;
    for adv in advisories {
        if let Ok(req) = semver::VersionReq::parse(&adv.affected_range) {
            if req.matches(&old_ver) {
                old_count += 1;
            }
            if req.matches(&new_ver) {
                new_count += 1;
            }
        }
    }
    Ok((old_count, new_count))
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
