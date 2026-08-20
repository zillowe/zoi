use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use colored::Colorize;
use dialoguer::MultiSelect;
use dialoguer::theme::ColorfulTheme;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use semver::Version;
use serde_json::json;

use crate::cmd::{utils as cmd_utils, ux};
use crate::pkg::merge::handle_backup_files;
use crate::pkg::{
    config, db, hooks, install, local, pin, resolve, transaction, types
};

/// The primary high-level orchestration for the `zoi update` command.
///
/// This function handles:
/// - Batch Updates: When `--all` is specified, it scans all installed packages.
/// - Targeted Updates: Updates specific packages provided by name.
/// - Advisory Deltas: Calculates and displays changes in security
///   vulnerabilities.
/// - Cleanup: Automatically removes old versions after a successful upgrade (if
///   rollbacks are not required).
///
/// # Errors
///
/// Returns an error if the update process fails for any package.
pub fn run(
    all: bool,
    package_names: &[String],
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
    verbose: bool,
    interactive: bool
) -> Result<()> {
    if all {
        return run_update_all_logic(
            yes,
            dry_run,
            explain,
            plan_json,
            verbose,
            interactive
        );
    }

    if plan_json && !dry_run {
        return Err(anyhow!("--plan-json requires --dry-run"));
    }

    let expanded_package_names =
        cmd_utils::expand_split_packages(package_names, "Updating")?;

    let mut failed_packages = Vec::new();

    for (i, package_name) in expanded_package_names.iter().enumerate() {
        if i > 0 {
            println!();
        }
        if let Err(e) = run_update_single_logic(
            package_name,
            yes,
            dry_run,
            explain,
            plan_json,
            verbose
        ) {
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

/// Logic for updating a single package.
fn run_update_single_logic(
    package_name: &str,
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
    verbose: bool
) -> Result<()> {
    if !plan_json {
        println!("{} Resolving dependencies...", "::".bold().blue());
    }

    let (new_pkg, new_version, _, _, registry_handle, _, _) =
        resolve::resolve_package_and_version(package_name, None, true, yes)?;

    if pin::is_pinned(package_name)? {
        if !plan_json {
            println!(
                "Package '{}' is pinned. Skipping update.",
                package_name.yellow()
            );
        }
        return Ok(());
    }

    let installed_source = if let Some(sub) = &new_pkg.sub_package {
        format!(
            "#{}@{}/{}:{}",
            registry_handle.as_deref().unwrap_or("local"),
            new_pkg.repo,
            new_pkg.name,
            sub
        )
    } else {
        format!(
            "#{}@{}/{}",
            registry_handle.as_deref().unwrap_or("local"),
            new_pkg.repo,
            new_pkg.name
        )
    };
    let installed_request = resolve::parse_source_string(&installed_source)?;
    let mut candidates = Vec::new();
    candidates.extend(local::find_installed_manifests_matching(
        &installed_request,
        types::Scope::User
    )?);
    candidates.extend(local::find_installed_manifests_matching(
        &installed_request,
        types::Scope::System
    )?);

    let old_manifest = crate::cmd::installed_select::choose_installed_manifest(
        package_name,
        &candidates,
        yes
    )
    .map_err(|e| {
        if candidates.is_empty() {
            anyhow!(
                "Package '{package_name}' is not installed. Use 'zoi install' \
                 instead."
            )
        } else {
            e
        }
    })?;

    if old_manifest.version == new_version
        && old_manifest.revision == new_pkg.revision
    {
        if !plan_json {
            println!("\nPackage is already up to date.");
            ux::print_transaction_summary(&ux::TransactionSummary {
                command: "update".to_string(),
                success: 0,
                failed: 0,
                skipped: 1
            });
        }
        return Ok(());
    }

    if !plan_json {
        println!("{} Looking for conflicts...", "::".bold().blue());
    }

    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &[package_name.to_string()],
        Some(old_manifest.scope),
        true,
        yes,
        false,
        None,
        true,
        None
    )?;

    if !plan_json {
        println!("{} Checking available disk space...", "::".bold().blue());
    }

    if !dry_run {
        let pkgs_to_check: Vec<&types::Package> =
            graph.nodes.values().map(|n| &n.pkg).collect();
        install::preflight::check_for_conflicts(&pkgs_to_check, yes)?;
        for pkg in &pkgs_to_check {
            if !install::util::display_updates(pkg, yes)? {
                return Err(anyhow!("Update aborted by user."));
            }
        }
        install::preflight::check_policy_compliance(&graph)?;
        install::preflight::check_scope_compliance(&graph)?;
        install::preflight::check_zoios_compliance(&graph)?;
        install::preflight::check_for_vulnerabilities(&graph, yes)?;

        let m_for_conflict_check = MultiProgress::new();
        install::preflight::check_file_conflicts(
            &graph,
            yes,
            &m_for_conflict_check
        )?;
        let _ = m_for_conflict_check.clear();
    }

    let install_plan =
        install::plan::create_install_plan(&graph.nodes, None, false)?;

    let mut total_download_size: u64 = 0;
    let mut total_installed_size_diff: i64 = 0;
    let mut packages_to_upgrade = Vec::new();
    let config = config::read_config().unwrap_or_default();

    for (pkg_id, node) in &graph.nodes {
        let is_requested = node.pkg.name == new_pkg.name
            && node.sub_package == old_manifest.sub_package;

        let request_source = local::package_source_string(
            &node.registry_handle,
            &node.pkg.repo,
            &node.pkg.name,
            node.sub_package.as_deref(),
            &node.version
        );
        let request = resolve::parse_source_string(&request_source)?;
        let installed = local::find_installed_manifests_matching(
            &request,
            old_manifest.scope
        )?;

        let (current_version_display, needs_update) = if installed.is_empty() {
            if is_requested {
                let display = if old_manifest.revision == "1" {
                    old_manifest.version.clone()
                } else {
                    format!(
                        "{}-{}",
                        old_manifest.version, old_manifest.revision
                    )
                };
                (Some(display), true)
            } else {
                (None, true)
            }
        } else {
            let m = installed
                .first()
                .ok_or_else(|| anyhow!("installed list unexpectedly empty"))?;
            let display = if m.revision == "1" {
                m.version.clone()
            } else {
                format!("{}-{}", m.version, m.revision)
            };
            (
                Some(display),
                m.version != node.version || m.revision != node.revision
            )
        };

        if needs_update {
            let (down_size, inst_size) = install::util::get_package_sizes(
                &node.pkg,
                &node.registry_handle,
                &node.version
            );
            total_download_size += down_size;

            let old_size = installed
                .first()
                .and_then(|m| m.installed_size)
                .unwrap_or(0);
            total_installed_size_diff += inst_size as i64 - old_size as i64;

            let display_name = ux::format_display_name(
                &node.registry_handle,
                &node.pkg.repo,
                &node.pkg.name,
                node.sub_package.as_deref(),
                &config
            );

            let new_version_display = if node.revision == "1" {
                node.version.clone()
            } else {
                format!("{}-{}", node.version, node.revision)
            };

            packages_to_upgrade.push((
                display_name,
                current_version_display,
                new_version_display,
                pkg_id.clone(),
                !is_requested
            ));
        }
    }

    if !plan_json {
        println!(
            "\n{} Packages ({})\n",
            "::".bold().blue(),
            packages_to_upgrade.len()
        );
    }

    for (name, old_ver, new_ver, pkg_id, is_dep) in &packages_to_upgrade {
        let transition = if let Some(old) = old_ver {
            format!(
                "{} -> {}",
                format!("{name}@{old}").cyan(),
                format!("{name}@{new_ver}").cyan()
            )
        } else {
            format!("{} (new dependency)", format!("{name}@{new_ver}").cyan())
        };
        if !plan_json {
            println!("  {transition}");
        }

        if *is_dep {
            let node = graph.nodes.get(pkg_id).ok_or_else(|| {
                anyhow!("Package node '{pkg_id}' missing from graph")
            })?;
            let pkg_dir = local::get_package_dir(
                old_manifest.scope,
                &node.registry_handle,
                &node.pkg.repo,
                &node.pkg.name
            )?;
            if let Ok(dependents) = local::get_dependents(&pkg_dir) {
                let external_dependents: Vec<_> = dependents
                    .into_iter()
                    .filter(|dep_id| !graph.nodes.contains_key(dep_id))
                    .collect();

                if !external_dependents.is_empty() && !plan_json {
                    println!(
                        "   {} Updating {} may affect: {}",
                        "Warning:".bold().yellow(),
                        node.pkg.name.cyan(),
                        external_dependents.join(", ").dimmed()
                    );
                }
            }
        }
    }

    let down_str = zoi_core::utils::format_bytes(total_download_size);
    let net_str = zoi_core::utils::format_size_diff(total_installed_size_diff);

    if !plan_json {
        println!("\nTotal Download Size: {down_str:>10}");
        println!("Net Upgrade Size:    {net_str:>10}");
    }

    if verbose && !plan_json {
        let preflight = ux::PreflightSummary::new("Update preflight")
            .row("Candidates", packages_to_upgrade.len().to_string())
            .row("Scope", format!("{:?}", old_manifest.scope))
            .row("Download size", &down_str)
            .row("Net size", &net_str);
        ux::print_preflight(&preflight);
    }

    if explain && !plan_json {
        let mut report = ux::ExplainReport::new("Update explanation");
        report = report.item(
            new_pkg.name.clone(),
            format!(
                "selected because newer version {} is available over \
                 installed {}",
                new_version, old_manifest.version
            ),
            Vec::new()
        );
        if let Ok((old_adv, new_adv)) = advisory_counts(
            &old_manifest.registry_handle,
            &new_pkg.name,
            old_manifest.sub_package.as_deref(),
            &old_manifest.version,
            &new_version
        ) {
            report = report.item(
                "advisories",
                format!(
                    "old={}, new={}, delta={}",
                    old_adv,
                    new_adv,
                    (new_adv as i64 - old_adv as i64)
                ),
                Vec::new()
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
                "download_bytes": total_download_size,
                "net_size_bytes": total_installed_size_diff,
            }
        });
        ux::emit_plan_json_v1("update", plan)?;
        return Ok(());
    }

    if dry_run {
        println!(
            "\n{} Dry-run: update plan above would be executed.",
            "::".bold().yellow()
        );
        return Ok(());
    }

    println!();
    let prompt = if packages_to_upgrade.len() > 1 {
        "Do you want to upgrade these packages?".to_string()
    } else {
        format!("Update from {} to {}?", old_manifest.version, new_version)
    };

    if !crate::utils::ask_for_confirmation(&prompt, yes) {
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: 0,
            skipped: packages_to_upgrade.len()
        });
        return Ok(());
    }

    perform_transaction(
        &graph,
        &install_plan,
        &non_zoi_deps,
        old_manifest.scope,
        yes,
        verbose,
        &new_pkg.name,
        &old_manifest,
        &new_pkg,
        plan_json
    )
}

/// Executes the transactional part of the update process.
///
/// This involves creating a transaction, installing new package versions,
/// recording the operations, and committing the transaction.
fn perform_transaction(
    graph: &install::resolver::DependencyGraph,
    install_plan: &HashMap<String, install::plan::InstallAction>,
    non_zoi_deps: &[String],
    scope: types::Scope,
    yes: bool,
    verbose: bool,
    target_pkg_name: &str,
    old_manifest_ref: &types::InstallManifest,
    new_pkg_ref: &types::Package,
    plan_json: bool
) -> Result<()> {
    let mut transaction = transaction::begin()?;
    let stages = graph.toposort()?;
    let mut new_manifest_option: Option<types::InstallManifest> = None;
    let m = MultiProgress::new();
    if plan_json {
        m.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    }

    // Pre-install dependencies (non-zoi)
    if !non_zoi_deps.is_empty() {
        let processed_deps = Mutex::new(HashSet::new());
        let mut installed_deps_ext = Vec::new();
        for dep_str in non_zoi_deps {
            let dep =
                crate::pkg::dependencies::parse_dependency_string(dep_str)?;
            crate::pkg::install::dep_install::install_dependency(
                &dep,
                "update",
                scope,
                yes,
                false,
                &processed_deps,
                &mut installed_deps_ext,
                Some(&m)
            )?;
        }
    }

    for stage in stages {
        for pkg_id in stage {
            let node = graph.nodes.get(&pkg_id).ok_or_else(|| {
                anyhow!("Package node '{pkg_id}' missing from graph")
            })?;
            if let Some(action) = install_plan.get(&pkg_id) {
                match install::installer::install_node(
                    node,
                    action,
                    Some(&m),
                    None,
                    yes,
                    true,
                    true,
                    verbose
                ) {
                    Ok(m) => {
                        if m.name == target_pkg_name {
                            new_manifest_option = Some(m);
                        }
                    }
                    Err(e) => {
                        eprintln!("\nError: Update failed. Rolling back...");
                        transaction::rollback(&transaction.id)?;
                        return Err(anyhow!("Update failed: {e}"));
                    }
                }
            }
        }
    }

    if let Some(new_manifest) = new_manifest_option {
        // Record and commit
        transaction::record_operation(
            &mut transaction,
            types::TransactionOperation::Upgrade {
                old_manifest: Box::new(old_manifest_ref.clone()),
                new_manifest: Box::new(new_manifest.clone())
            }
        )?;

        if let Ok(modified_files) =
            transaction::get_modified_files(&transaction.id)
        {
            let modified_packages =
                transaction::get_modified_packages(&transaction.id)
                    .unwrap_or_default();
            let _ = crate::pkg::hooks::global::run_global_hooks(
                crate::pkg::hooks::global::HookWhen::PostTransaction,
                &modified_files,
                &modified_packages,
                "upgrade",
                scope
            );
        }

        transaction::commit(&transaction.id)?;

        if let Some(backup_files) = &old_manifest_ref.backup {
            println!("Restoring configuration files...");
            let old_version_dir = local::get_package_version_dir(
                old_manifest_ref.scope,
                &old_manifest_ref.registry_handle,
                &old_manifest_ref.repo,
                &old_manifest_ref.name,
                &old_manifest_ref.version
            )?;
            let new_version_dir = local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version
            )?;

            crate::pkg::merge::handle_backup_files(
                &old_version_dir,
                &new_version_dir,
                backup_files,
                old_manifest_ref.scope
            )?;
        }

        // Cleanup and finish
        cleanup_old_versions(
            &new_manifest.name,
            scope,
            &new_manifest.repo,
            &new_manifest.registry_handle
        )?;

        if let Ok(conn) = db::open_connection("local") {
            let _ = db::update_package(
                &conn,
                new_pkg_ref,
                &new_manifest.registry_handle,
                Some(new_manifest.scope),
                new_manifest.sub_package.as_deref(),
                Some(&types::InstallReason::Direct)
            );
        }

        if let Some(hooks) = &new_pkg_ref.hooks {
            crate::pkg::hooks::run_hooks(
                hooks,
                crate::pkg::hooks::HookType::PostUpgrade,
                new_manifest.scope
            )?;
        }

        if !plan_json {
            println!("\n{}", "Success:".green());
        }
    }

    Ok(())
}

/// Logic for updating all installed packages.
fn run_update_all_logic(
    yes: bool,
    dry_run: bool,
    explain: bool,
    plan_json: bool,
    verbose: bool,
    interactive: bool
) -> Result<()> {
    #[derive(Clone)]
    struct UpdateCandidate {
        source: String,
        new_pkg: types::Package,
        new_version: String,
        old_manifest: types::InstallManifest,
        old_advisories: usize,
        new_advisories: usize,
        download_size: u64,
        new_installed_size: u64
    }

    let installed_packages = local::get_installed_packages()?;
    let mut pinned_sources = Vec::new();
    let mut skipped_sources = Vec::new();
    let mut up_to_date_sources = Vec::new();
    let mut candidates: Vec<UpdateCandidate> = Vec::new();

    // --- Phase 1: Upgrade Scanning ---
    // We scan all installed packages and compare them against the latest
    // registry metadata.
    if !plan_json {
        println!("{} Checking for upgrades...", "::".bold().blue());
    }
    let pb = ProgressBar::new(installed_packages.len() as u64);
    if plan_json {
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    }
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                 {pos}/{len} ({msg})"
            )?
            .progress_chars("#>-")
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
            pinned_sources.push(source);
            pb.inc(1);
            continue;
        }

        let (new_pkg, new_version, _, _, _registry_handle, _, _) =
            match resolve::resolve_package_and_version(
                &source,
                Some(manifest.scope),
                true,
                false
            ) {
                Ok(result) => result,
                Err(e) => {
                    skipped_sources.push(format!("{source} ({e})"));
                    pb.inc(1);
                    continue;
                }
            };

        if manifest.version == new_version
            && manifest.revision == new_pkg.revision
        {
            up_to_date_sources.push(source);
            pb.inc(1);
            continue;
        }

        let (old_adv, new_adv) = advisory_counts(
            &manifest.registry_handle,
            &manifest.name,
            manifest.sub_package.as_deref(),
            &manifest.version,
            &new_version
        )
        .unwrap_or((0, 0));

        let (download_size, new_installed_size) =
            install::util::get_package_sizes(
                &new_pkg,
                &manifest.registry_handle,
                &new_version
            );

        candidates.push(UpdateCandidate {
            source,
            new_pkg,
            new_version,
            old_manifest: manifest,
            old_advisories: old_adv,
            new_advisories: new_adv,
            download_size,
            new_installed_size
        });
        pb.inc(1);
    }
    pb.finish_and_clear();

    if candidates.is_empty() {
        if !plan_json {
            println!("\nAll packages are up to date.");
        }
        return Ok(());
    }

    if interactive && !dry_run {
        let items: Vec<String> = candidates
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
            .map_err(|e| anyhow!("Interactive selection failed: {e}"))?;

        if selected.is_empty() {
            if !plan_json {
                println!("No packages selected.");
            }
            return Ok(());
        }

        let selected_set: HashSet<usize> = selected.into_iter().collect();
        candidates = candidates
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| selected_set.contains(idx))
            .map(|(_, c)| c)
            .collect();
    }

    if !plan_json {
        println!("{} Resolving dependencies...", "::".bold().blue());
        println!("{} Looking for conflicts...", "::".bold().blue());
        println!("{} Checking available disk space...", "::".bold().blue());

        if !pinned_sources.is_empty() {
            println!("\n{} Pinned (skipped)", "::".bold().blue());
            for s in &pinned_sources {
                println!("  - {}", s.yellow());
            }
        }

        println!("\n{} Packages ({})", "::".bold().blue(), candidates.len());
    }
    let config = config::read_config().unwrap_or_default();

    for candidate in &candidates {
        let delta =
            candidate.new_advisories as i64 - candidate.old_advisories as i64;
        let advisory_suffix = match delta.cmp(&0) {
            std::cmp::Ordering::Greater => {
                format!(" (advisories +{delta})").red().to_string()
            }
            std::cmp::Ordering::Less => {
                format!(" (advisories {delta})").green().to_string()
            }
            std::cmp::Ordering::Equal => String::new()
        };

        let display_name = ux::format_display_name(
            &candidate.old_manifest.registry_handle,
            &candidate.old_manifest.repo,
            &candidate.old_manifest.name,
            candidate.old_manifest.sub_package.as_deref(),
            &config
        );
        let old_display = if candidate.old_manifest.revision == "1" {
            candidate.old_manifest.version.clone()
        } else {
            format!(
                "{}-{}",
                candidate.old_manifest.version, candidate.old_manifest.revision
            )
        };
        let new_display = if candidate.new_pkg.revision == "1" {
            candidate.new_version.clone()
        } else {
            format!("{}-{}", candidate.new_version, candidate.new_pkg.revision)
        };

        if !plan_json {
            println!(
                "  {} -> {}{}",
                format!("{display_name}@{old_display}").cyan(),
                format!("{display_name}@{new_display}").cyan(),
                advisory_suffix
            );
        }
    }

    let total_download_size: u64 =
        candidates.iter().map(|c| c.download_size).sum();
    let total_installed_size_diff: i64 = candidates
        .iter()
        .map(|c| {
            c.new_installed_size as i64
                - c.old_manifest.installed_size.unwrap_or(0) as i64
        })
        .sum();

    let down_str = zoi_core::utils::format_bytes(total_download_size);
    let net_str = zoi_core::utils::format_size_diff(total_installed_size_diff);

    if !plan_json {
        println!("\nTotal Download Size: {down_str:>10}");
        println!("Net Upgrade Size:    {net_str:>10}");
    }

    if verbose && !plan_json {
        let preflight = ux::PreflightSummary::new("Update preflight")
            .row("Candidates", candidates.len().to_string())
            .row("Download size", &down_str)
            .row("Net size", &net_str);
        ux::print_preflight(&preflight);
    }

    if explain && !plan_json {
        let mut report = ux::ExplainReport::new("Update explanation");
        for candidate in &candidates {
            report = report.item(
                candidate.source.clone(),
                format!(
                    "selected because {} -> {}",
                    candidate.old_manifest.version, candidate.new_version
                ),
                Vec::new()
            );
        }
        ux::print_explain(&report);
    }

    if plan_json {
        let packages: Vec<_> = candidates
            .iter()
            .map(|c| {
                json!({
                    "source": c.source,
                    "name": c.new_pkg.name,
                    "sub_package": c.old_manifest.sub_package,
                    "from_version": c.old_manifest.version,
                    "to_version": c.new_version,
                    "download_bytes": c.download_size,
                    "net_size_bytes": c.new_installed_size as i64 - c.old_manifest.installed_size.unwrap_or(0) as i64,
                    "advisories_old": c.old_advisories,
                    "advisories_new": c.new_advisories,
                })
            })
            .collect();

        let plan = json!({
            "dry_run": dry_run,
            "interactive": interactive,
            "totals": {
                "candidates": candidates.len(),
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
        return Ok(());
    }

    if dry_run {
        println!(
            "\n{} Dry-run: upgrade plan above would be executed.",
            "::".bold().yellow()
        );
        return Ok(());
    }

    if !crate::utils::ask_for_confirmation(
        "Do you want to upgrade these packages?",
        yes
    ) {
        return Ok(());
    }

    // --- Phase 2: Transactional Upgrade ---
    let transaction = Mutex::new(transaction::begin()?);
    let transaction_id = transaction.lock().expect("mutex poisoned").id.clone();
    let failed_updates = Mutex::new(Vec::new());
    let successful_upgrades = Mutex::new(Vec::new());

    let m = MultiProgress::new();

    candidates
        .par_iter()
        .try_for_each(|candidate| -> Result<()> {
            println!(
                "\n{} Upgrading {} to {}...",
                "::".bold().blue(),
                candidate.source.cyan(),
                candidate.new_version.green()
            );

            if let Some(hooks) = &candidate.new_pkg.hooks {
                hooks::run_hooks(
                    hooks,
                    hooks::HookType::PreUpgrade,
                    candidate.old_manifest.scope
                )?;
            }

            let (graph, _) = match install::resolver::resolve_dependency_graph(
                std::slice::from_ref(&candidate.source),
                Some(candidate.old_manifest.scope),
                true,
                yes,
                false,
                None,
                false,
                None
            ) {
                Ok(res) => res,
                Err(e) => {
                    eprintln!(
                        "{}: Failed to resolve dependencies for '{}': {}",
                        "Error".red().bold(),
                        candidate.source,
                        e
                    );
                    failed_updates
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                        .push(candidate.source.clone());
                    return Ok(());
                }
            };

            if let Err(e) =
                zoi_install::preflight::check_policy_compliance(&graph)
            {
                eprintln!(
                    "{}: Policy check failed for '{}': {}",
                    "Error".red().bold(),
                    candidate.source,
                    e
                );
                failed_updates
                    .lock()
                    .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                    .push(candidate.source.clone());
                return Ok(());
            }

            if let Err(e) =
                zoi_install::preflight::check_scope_compliance(&graph)
            {
                eprintln!(
                    "{}: Scope check failed for '{}': {}",
                    "Error".red().bold(),
                    candidate.source,
                    e
                );
                failed_updates
                    .lock()
                    .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                    .push(candidate.source.clone());
                return Ok(());
            }

            if let Err(e) =
                zoi_install::preflight::check_zoios_compliance(&graph)
            {
                eprintln!(
                    "{}: ZoiOS check failed for '{}': {}",
                    "Error".red().bold(),
                    candidate.source,
                    e
                );
                failed_updates
                    .lock()
                    .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                    .push(candidate.source.clone());
                return Ok(());
            }

            if let Err(e) =
                zoi_install::preflight::check_for_vulnerabilities(&graph, yes)
            {
                eprintln!(
                    "{}: Security check failed for '{}': {}",
                    "Error".red().bold(),
                    candidate.source,
                    e
                );
                failed_updates
                    .lock()
                    .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                    .push(candidate.source.clone());
                return Ok(());
            }

            let install_plan = match install::plan::create_install_plan(
                &graph.nodes,
                None,
                false
            ) {
                Ok(plan) => plan,
                Err(e) => {
                    eprintln!(
                        "{}: Failed to create install plan for '{}': {}",
                        "Error".red().bold(),
                        candidate.source,
                        e
                    );
                    failed_updates
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                        .push(candidate.source.clone());
                    return Ok(());
                }
            };

            let stages = match graph.toposort() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "{}: Failed to sort dependency graph for '{}': {}",
                        "Error".red().bold(),
                        candidate.source,
                        e
                    );
                    failed_updates
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                        .push(candidate.source.clone());
                    return Ok(());
                }
            };

            let mut new_manifest_option: Option<types::InstallManifest> = None;
            for stage in stages {
                for pkg_id in stage {
                    let node = graph.nodes.get(&pkg_id).ok_or_else(|| {
                        anyhow!(
                            "Package node '{pkg_id}' missing from graph \
                             during update"
                        )
                    })?;
                    if let Some(action) = install_plan.get(&pkg_id) {
                        match install::installer::install_node(
                            node,
                            action,
                            Some(&m),
                            None,
                            yes,
                            true,
                            true,
                            false
                        ) {
                            Ok(m) => {
                                if m.name == candidate.new_pkg.name
                                    && m.sub_package
                                        == candidate.old_manifest.sub_package
                                {
                                    new_manifest_option = Some(m);
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "{}: Failed to upgrade '{}': {}",
                                    "Error".red().bold(),
                                    candidate.source,
                                    e
                                );
                                failed_updates
                                    .lock()
                                    .map_err(|e| {
                                        anyhow!("mutex poisoned: {e}")
                                    })?
                                    .push(candidate.source.clone());
                                return Ok(());
                            }
                        }
                    }
                }
            }

            if let Some(new_manifest) = new_manifest_option {
                if let Err(e) = transaction::record_operation(
                    &mut *transaction
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?,
                    types::TransactionOperation::Upgrade {
                        old_manifest: Box::new(candidate.old_manifest.clone()),
                        new_manifest: Box::new(new_manifest.clone())
                    }
                ) {
                    eprintln!(
                        "Error: Failed to record transaction for {}: {}",
                        candidate.source, e
                    );
                    failed_updates
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                        .push(candidate.source.clone());
                } else {
                    successful_upgrades
                        .lock()
                        .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                        .push((
                            candidate.old_manifest.clone(),
                            new_manifest.clone(),
                            candidate.new_pkg.clone()
                        ));
                }
            } else {
                eprintln!(
                    "Failed to get new manifest for {}",
                    candidate.source
                );
                failed_updates
                    .lock()
                    .map_err(|e| anyhow!("mutex poisoned: {e}"))?
                    .push(candidate.source.clone());
            }
            Ok(())
        })?;

    let failed = failed_updates
        .into_inner()
        .map_err(|e| anyhow!("mutex poisoned: {e}"))?;
    if !failed.is_empty() {
        eprintln!(
            "\nError: Some packages failed to upgrade. Rolling back all \
             changes..."
        );
        for pkg in &failed {
            eprintln!("  - {pkg}");
        }
        transaction::rollback(&transaction_id)?;
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: 0,
            failed: failed.len(),
            skipped: pinned_sources.len()
                + skipped_sources.len()
                + up_to_date_sources.len()
        });
        return Err(anyhow!("Update failed for some packages."));
    }

    if let Ok(modified_files) = transaction::get_modified_files(&transaction_id)
    {
        let modified_packages =
            transaction::get_modified_packages(&transaction_id)
                .unwrap_or_default();
        let upgrades_lock = successful_upgrades
            .lock()
            .map_err(|e| anyhow!("mutex poisoned: {e}"))?;
        let first_scope = upgrades_lock
            .first()
            .map_or(types::Scope::User, |(old, _, _)| old.scope);
        let _ = crate::pkg::hooks::global::run_global_hooks(
            crate::pkg::hooks::global::HookWhen::PostTransaction,
            &modified_files,
            &modified_packages,
            "upgrade",
            first_scope
        );
    }
    transaction::commit(&transaction_id)?;

    if !plan_json {
        println!("\n{}", "Success:".green());
    }
    let successful_upgrades = successful_upgrades
        .into_inner()
        .map_err(|e| anyhow!("mutex poisoned: {e}"))?;
    for (old_manifest, new_manifest, new_pkg) in &successful_upgrades {
        if let Some(backup_files) = &old_manifest.backup {
            if !plan_json {
                println!(
                    "Restoring configuration for {}...",
                    old_manifest.name.cyan()
                );
            }
            let old_version_dir = local::get_package_version_dir(
                old_manifest.scope,
                &old_manifest.registry_handle,
                &old_manifest.repo,
                &old_manifest.name,
                &old_manifest.version
            )?;
            let new_version_dir = local::get_package_version_dir(
                new_manifest.scope,
                &new_manifest.registry_handle,
                &new_manifest.repo,
                &new_manifest.name,
                &new_manifest.version
            )?;
            handle_backup_files(
                &old_version_dir,
                &new_version_dir,
                backup_files,
                old_manifest.scope
            )?;
        }

        if let Err(e) = cleanup_old_versions(
            &new_manifest.name,
            new_manifest.scope,
            &new_manifest.repo,
            &new_manifest.registry_handle
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
                Some(&old_manifest.reason)
            );
        }

        if let Some(hooks) = &new_pkg.hooks
            && let Err(e) = hooks::run_hooks(
                hooks,
                hooks::HookType::PostUpgrade,
                new_manifest.scope
            )
        {
            eprintln!(
                "{}: Post-upgrade hook failed for '{}': {}",
                "Error".red().bold(),
                new_manifest.name,
                e
            );
        }
    }

    if !plan_json {
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "update".to_string(),
            success: successful_upgrades.len(),
            failed: 0,
            skipped: pinned_sources.len()
                + skipped_sources.len()
                + up_to_date_sources.len()
        });
    }
    Ok(())
}

/// Calculates the number of security advisories for a package at two different
/// versions.
fn advisory_counts(
    registry_handle: &str,
    package: &str,
    sub_package: Option<&str>,
    old_version: &str,
    new_version: &str
) -> Result<(usize, usize)> {
    let advisories =
        db::get_advisories_for_package(registry_handle, package, sub_package)?;
    let old_ver = Version::parse(old_version).map_err(|e| {
        anyhow!("failed to parse old version '{old_version}': {e}")
    })?;
    let new_ver = Version::parse(new_version).map_err(|e| {
        anyhow!("failed to parse new version '{new_version}': {e}")
    })?;

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

/// Removes old versions of a package, keeping a limited number for potential
/// rollbacks.
fn cleanup_old_versions(
    package_name: &str,
    scope: types::Scope,
    repo: &str,
    registry_handle: &str
) -> Result<()> {
    let config = config::read_config()?;
    let rollback_enabled = config.rollback_enabled;
    let package_dir =
        local::get_package_dir(scope, registry_handle, repo, package_name)?;

    let mut versions = Vec::new();
    if let Ok(entries) = fs::read_dir(&package_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(version_str) =
                    path.file_name().and_then(|s| s.to_str())
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

    let versions_to_keep = if rollback_enabled { 3 } else { 1 };
    if versions.len() > versions_to_keep {
        let num_to_delete = versions.len() - versions_to_keep;
        println!("Cleaning up old versions...");
        let to_delete = versions
            .get(..num_to_delete)
            .ok_or_else(|| anyhow!("Version slice out of bounds"))?;
        for version in to_delete {
            let version_dir_to_delete = package_dir.join(version.to_string());
            println!(" - Removing {}", version_dir_to_delete.display());
            let _ = fs::remove_dir_all(version_dir_to_delete);
        }
    }
    Ok(())
}
