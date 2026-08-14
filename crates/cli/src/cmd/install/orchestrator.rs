//! High-level installation orchestration.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use mlua::LuaSerdeExt;
use rayon::prelude::*;
use serde_json::json;
use zoi_core::{lock, types};
use zoi_install::{installer, lockfile, plan, preflight, resolver, util};
use zoi_plugins::PluginManager;
use zoi_project as project;
use zoi_resolver::local;
use zoi_transaction as transaction;

use crate::utils as cli_utils;

/// Options for the installation orchestrator.
pub struct InstallOptions<'a> {
    /// The scope to install the packages into.
    pub scope: types::Scope,
    /// Whether to force the installation.
    pub force: bool,
    /// Whether to install all optional dependencies.
    pub all_optional: bool,
    /// Whether to skip confirmation prompts.
    pub yes: bool,
    /// Whether to save the installation to the project file.
    pub save: bool,
    /// The build type to use.
    pub build_type: Option<&'a str>,
    /// Whether to perform a dry run.
    pub dry_run: bool,
    /// The plugin manager to use.
    pub plugin_manager: Option<&'a PluginManager>,
    /// Whether to force building from source.
    pub build: bool,
    /// Whether to use the lockfile exactly (frozen).
    pub frozen: bool,
    /// Whether to explain decisions.
    pub explain: bool,
    /// Whether to emit machine-readable plan JSON.
    pub plan_json: bool,
    /// Number of download retry attempts.
    pub retry: u32,
    /// Whether to show verbose output.
    pub verbose: bool,
    /// Whether to use PURL (Package URL) specification.
    pub purl: bool,
    /// Optional project configuration override.
    pub project_config: Option<project::config::ProjectConfig>
}

/// The installation orchestrator.
pub struct Orchestrator<'a> {
    /// The options used for the installation.
    options: InstallOptions<'a>
}

impl<'a> Orchestrator<'a> {
    /// Creates a new orchestrator with the given options.
    pub fn new(options: InstallOptions<'a>) -> Self {
        Self { options }
    }

    /// Runs the installation for the given sources.
    ///
    /// # Errors
    ///
    /// Returns an error if the installation fails at any stage.
    ///
    /// # Panics
    ///
    /// Panics if any of the internal mutexes (failed packages, prepared nodes,
    /// etc.) are poisoned.
    pub fn run(&self, sources: &[String], repo: Option<String>) -> Result<()> {
        let options = &self.options;
        util::set_download_retry_attempts(options.retry);

        if sources.is_empty() && repo.is_none() && !options.frozen {
            return Err(anyhow!("No packages specified for installation."));
        }

        // 1. Initial lock and setup
        let _lock = lock::acquire_lock()?;

        let mut scope_override = Some(options.scope);

        if options.frozen {
            if repo.is_some() || !sources.is_empty() {
                return Err(anyhow!(
                    "--frozen can only be used without explicit sources or \
                     --repo."
                ));
            }
            if options.save {
                return Err(anyhow!(
                    "--save cannot be used with --frozen because the lockfile \
                     must remain unchanged."
                ));
            }
            if !std::path::Path::new("zoi.lua").exists() {
                return Err(anyhow!(
                    "--frozen requires a local zoi.lua in the current project."
                ));
            }
            if !std::path::Path::new("zoi.lock").exists() {
                return Err(anyhow!(
                    "--frozen requires zoi.lock. Generate it first with a \
                     normal project install."
                ));
            }
            if let Some(scope) = scope_override
                && scope != types::Scope::Project
            {
                return Err(anyhow!(
                    "--frozen is only supported for project scope installs."
                ));
            }
            scope_override = Some(types::Scope::Project);
            zoi_core::frozen::set_frozen(true);
        }

        let lockfile_exists = sources.is_empty()
            && repo.is_none()
            && std::path::Path::new("zoi.lock").exists()
            && (std::path::Path::new("zoi.lua").exists()
                || std::path::Path::new("zoi.yaml").exists());

        let mut sources_to_process: Vec<String> = sources.to_vec();
        let mut _is_project_install = false;
        let mut frozen_packages = None;

        if options.frozen {
            let lockfile = project::lockfile::read_zoi_lock()?;
            let locked_packages = project::lockfile::locked_packages(&lockfile);
            sources_to_process = locked_packages
                .iter()
                .map(|entry| entry.source.clone())
                .collect();
            if sources_to_process.is_empty() {
                return Err(anyhow!(
                    "zoi.lock is empty. Cannot continue with --frozen."
                ));
            }
            frozen_packages = Some(locked_packages);
            println!(
                "{} --frozen enabled. Installing pinned lockfile sources \
                 only...",
                "::".bold().blue()
            );
            _is_project_install = true;
        } else if sources.is_empty() && repo.is_none() {
            if std::path::Path::new("zoi.lua").exists()
                || std::path::Path::new("zoi.yaml").exists()
            {
                if let Ok(config) = project::config::load() {
                    let config_file =
                        if std::path::Path::new("zoi.lua").exists() {
                            "zoi.lua"
                        } else {
                            "zoi.yaml"
                        };
                    if lockfile_exists {
                        println!(
                            "{} zoi.lock found. Installing from {} then \
                             verifying...",
                            "::".bold().blue(),
                            config_file
                        );
                    } else {
                        println!(
                            "{} Installing project packages from {}...",
                            "::".bold().blue(),
                            config_file
                        );
                    }
                    sources_to_process.clone_from(&config.pkgs);
                    if scope_override.is_none() {
                        scope_override = Some(types::Scope::Project);
                    }
                    _is_project_install = true;
                }
            } else if let Some(pm) = options.plugin_manager
                && pm.trigger_project_install_hook()?
            {
                return Ok(());
            }
        }

        if let Some(_repo_spec) = repo {
            if scope_override == Some(types::Scope::Project) {
                return Err(anyhow!(
                    "Installing from a repository to a project scope is not \
                     supported."
                ));
            }

            return Err(anyhow!(
                "Repository installation not implemented in Orchestrator yet."
            ));
        }

        if sources_to_process.is_empty() {
            return Ok(());
        }

        if options.purl {
            let mut resolved_purls = Vec::new();
            for source in &sources_to_process {
                println!(
                    "{} Fetching PURL package '{}'...",
                    "::".bold().blue(),
                    source
                );
                let ident = zoi_purl::fetch_and_store_purl_package(source)?;
                resolved_purls.push(ident);
            }
            sources_to_process = resolved_purls;
        }

        let config = zoi_core::config::read_config().unwrap_or_default();
        let jobs = config.jobs.unwrap_or(3);
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .ok();

        let failed_packages = Mutex::new(Vec::new());
        let mut temp_files = Vec::new();
        let mut final_sources = Vec::new();

        for source in &sources_to_process {
            if std::path::Path::new(source)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("lock"))
            {
                lockfile::process_lockfile(
                    source,
                    &mut final_sources,
                    &mut temp_files,
                    scope_override.unwrap_or(types::Scope::User)
                )?;
            } else {
                final_sources.push(source.clone());
            }
        }

        let successfully_installed_sources = Mutex::new(Vec::new());
        let installed_manifests = Mutex::new(Vec::new());

        // --- Phase 2: Dependency Resolution ---
        let (mut graph, mut non_zoi_deps) =
            if let Some(locked_packages) = frozen_packages.as_ref() {
                resolver::build_graph_from_locked_packages(
                    locked_packages,
                    scope_override,
                    false,
                    options.yes
                )?
            } else {
                resolver::resolve_dependency_graph(
                    &final_sources,
                    scope_override,
                    options.force,
                    options.yes,
                    options.all_optional,
                    options.build_type,
                    false,
                    options.project_config.clone()
                )?
            };

        let mut skipped_existing_count = 0usize;
        if !options.force {
            let mut to_remove = Vec::new();
            for (pkg_id, node) in &graph.nodes {
                let request_source = local::package_source_string(
                    &node.registry_handle,
                    &node.pkg.repo,
                    &node.pkg.name,
                    node.sub_package.as_deref(),
                    &node.version
                );
                let request = zoi_resolver::resolve::parse_source_string(
                    &request_source
                )?;
                let matches = local::find_installed_manifests_matching(
                    &request,
                    scope_override.unwrap_or(node.pkg.scope)
                )?;
                if matches
                    .iter()
                    .any(|manifest| manifest.version == node.version)
                {
                    println!(
                        "{} Package '{}' is already installed at version {}. \
                         Skipping.",
                        "::".bold().green(),
                        node.pkg.name.cyan(),
                        node.version.yellow()
                    );
                    to_remove.push(pkg_id.clone());
                }
            }
            skipped_existing_count = to_remove.len();

            for pkg_id in to_remove {
                graph.nodes.remove(&pkg_id);
                if let Some(children) = graph.adj.remove(&pkg_id)
                    && let Some(root_children) = graph.adj.get_mut("$root")
                {
                    for child in children {
                        root_children.insert(child);
                    }
                }
                if let Some(root_children) = graph.adj.get_mut("$root") {
                    root_children.remove(&pkg_id);
                }
            }

            let mut valid_non_zoi_deps = std::collections::HashSet::new();
            for source in &sources_to_process {
                if let Ok(dep) = zoi_deps::parse_dependency_string(source)
                    && dep.manager != "zoi"
                {
                    valid_non_zoi_deps.insert(source.clone());
                }
            }
            for node in graph.nodes.values() {
                for dep in &node.dependencies {
                    if let Ok(dep_req) = zoi_deps::parse_dependency_string(dep)
                        && dep_req.manager != "zoi"
                    {
                        valid_non_zoi_deps.insert(dep.clone());
                    }
                }
            }
            non_zoi_deps.retain(|dep| valid_non_zoi_deps.contains(dep));
        }

        if graph.nodes.is_empty() && non_zoi_deps.is_empty() {
            println!("\nAll requested packages are already installed.");
            return Ok(());
        }

        if !options.dry_run {
            if let Some(pm) = options.plugin_manager {
                pm.set_context(scope_override.unwrap_or_default())?;
            }
            for node in graph.nodes.values() {
                if let Some(pm) = options.plugin_manager {
                    let pkg_val = pm
                        .lua
                        .to_value(&node.pkg)
                        .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
                    pm.trigger_hook("on_pre_install", Some(&pkg_val))?;
                }
            }
        }

        let mut direct_packages = Vec::new();
        let mut dependencies = Vec::new();

        for node in graph.nodes.values() {
            if matches!(node.reason, types::InstallReason::Direct) {
                direct_packages.push(node);
            } else {
                dependencies.push(node);
            }
        }

        direct_packages.sort_by(|a, b| a.pkg.name.cmp(&b.pkg.name));
        dependencies.sort_by(|a, b| a.pkg.name.cmp(&b.pkg.name));

        for node in graph.nodes.values() {
            cli_utils::print_repo_warning(&node.pkg.repo);
        }

        // --- Phase 3: Safety & Compliance Checks ---
        println!("{} Looking for conflicts...", "::".bold().blue());
        let packages_to_install: Vec<&types::Package> =
            graph.nodes.values().map(|n| &n.pkg).collect();

        if !options.dry_run {
            preflight::check_for_conflicts(&packages_to_install, options.yes)?;
            for pkg in &packages_to_install {
                if !util::display_updates(pkg, options.yes)? {
                    return Err(anyhow!("Installation aborted by user."));
                }
            }
            preflight::check_policy_compliance(&graph)?;
            preflight::check_scope_compliance(&graph)?;
            preflight::check_zoios_compliance(&graph)?;
            preflight::check_for_vulnerabilities(&graph, options.yes)?;

            let m_for_conflict_check = MultiProgress::new();
            preflight::check_file_conflicts(
                &graph,
                options.yes,
                &m_for_conflict_check
            )?;
            let _ = m_for_conflict_check.clear();
        }

        println!("{} Checking available disk space...", "::".bold().blue());
        let install_plan = plan::create_install_plan(
            &graph.nodes,
            options.build_type,
            options.build
        )?;

        let mut total_download_size: u64 = 0;
        let mut total_installed_size: u64 = 0;
        let mut unique_downloads = HashSet::new();

        for (id, node) in &graph.nodes {
            match install_plan.get(id) {
                Some(plan::InstallAction::DownloadAndInstall(details)) => {
                    if unique_downloads.insert(details.info.final_url.clone()) {
                        total_download_size += details.download_size;
                    }
                    total_installed_size += if details.installed_size > 0 {
                        details.installed_size
                    } else {
                        node.pkg.installed_size.unwrap_or(0)
                    };
                }
                Some(plan::InstallAction::BuildAndInstall) => {
                    total_installed_size +=
                        node.pkg.installed_size.unwrap_or(0);
                }
                _ => {}
            }
        }

        if options.plan_json {
            let mut packages = Vec::new();
            for (id, node) in &graph.nodes {
                let action_name = match install_plan.get(id) {
                    Some(plan::InstallAction::DownloadAndInstall(_)) => {
                        "download"
                    }
                    Some(plan::InstallAction::InstallFromArchive(_)) => {
                        "archive"
                    }
                    Some(plan::InstallAction::BuildAndInstall) => "build",
                    None => "unknown"
                };
                let reason = match &node.reason {
                    types::InstallReason::Direct => "direct".to_string(),
                    types::InstallReason::Dependency { parent } => {
                        format!("dependency:{parent}")
                    }
                };
                packages.push(json!({
                    "id": id,
                    "name": node.pkg.name,
                    "version": node.version,
                    "revision": node.revision,
                    "sub_package": node.sub_package,
                    "repo": node.pkg.repo,
                    "registry": node.registry_handle,
                    "reason": reason,
                    "action": action_name,
                    "source": node.source,
                }));
            }

            let plan_data = json!({
                "dry_run": options.dry_run,
                "frozen": options.frozen,
                "retry_attempts": options.retry,
                "scope": format!("{:?}", scope_override.unwrap_or(types::Scope::User)),
                "totals": {
                    "direct_packages": direct_packages.len(),
                    "dependencies": dependencies.len() + non_zoi_deps.len(),
                    "download_bytes": total_download_size,
                    "installed_bytes": total_installed_size,
                    "skipped_existing": skipped_existing_count,
                },
                "packages": packages,
                "non_zoi_dependencies": non_zoi_deps,
            });
            println!("{}", serde_json::to_string_pretty(&plan_data)?);
        }

        if options.dry_run {
            println!(
                "\n{} Dry-run: installation plan above would be executed.",
                "::".bold().yellow()
            );
            return Ok(());
        }

        // --- Phase 4: Transactional Execution ---
        let install_path =
            local::get_store_base_dir(scope_override.unwrap_or_default())?;
        std::fs::create_dir_all(&install_path)?;

        let available_space =
            fs2::available_space(&install_path).unwrap_or(u64::MAX);

        if total_installed_size > available_space {
            return Err(anyhow!(
                "Not enough disk space. Required: {total_installed_size}, \
                 Available: {available_space}"
            ));
        }

        let yes = options.yes;
        if !zoi_core::utils::ask_for_confirmation(
            "\nProceed with installation?",
            yes
        ) {
            let _ = lock::release_lock();
            return Ok(());
        }

        let stages = graph.toposort()?;
        let transaction = Mutex::new(transaction::begin()?);
        let transaction_id = transaction
            .lock()
            .expect("Transaction mutex poisoned")
            .id
            .clone();
        let dependency_installed_count = AtomicUsize::new(0);

        println!("\n{} Preparing packages...", "::".bold().blue());
        let m_prep = MultiProgress::new();
        let prepared_nodes = Mutex::new(HashMap::new());

        let build_type = options.build_type;
        let verbose = options.verbose;

        stages
            .par_iter()
            .flatten()
            .try_for_each(|pkg_id| -> Result<()> {
                let node = graph.nodes.get(pkg_id).ok_or_else(|| {
                    anyhow!(
                        "Package node '{pkg_id}' missing from graph during \
                         preparation"
                    )
                })?;
                let action = install_plan.get(pkg_id).ok_or_else(|| {
                    anyhow!(
                        "Install action missing for package '{pkg_id}' during \
                         preparation"
                    )
                })?;

                let prepared = installer::prepare_node(
                    node,
                    action,
                    Some(&m_prep),
                    build_type,
                    verbose
                )?;

                let mut lock = prepared_nodes.lock().map_err(|e| {
                    anyhow!(
                        "Prepared nodes mutex poisoned during preparation: {e}"
                    )
                })?;
                lock.insert(pkg_id.clone(), prepared);
                Ok(())
            })?;

        if !dependencies.is_empty() || !non_zoi_deps.is_empty() {
            println!("\n{} Installing dependencies...", "::".bold().blue());
            let m_deps = MultiProgress::new();

            for stage in &stages {
                stage.par_iter().try_for_each(|pkg_id| -> Result<()> {
                    let node = graph.nodes.get(pkg_id).ok_or_else(|| {
                        anyhow!(
                            "Package node '{pkg_id}' missing from graph \
                             during installation"
                        )
                    })?;
                    if matches!(node.reason, types::InstallReason::Direct) {
                        return Ok(());
                    }

                    let prepared = {
                        let lock = prepared_nodes.lock().map_err(|e| {
                            anyhow!(
                                "Prepared nodes mutex poisoned during \
                                 dependency install: {e}"
                            )
                        })?;
                        lock.get(pkg_id).cloned().ok_or_else(|| {
                            anyhow!("Prepared node missing for: {pkg_id}")
                        })?
                    };

                    match installer::install_prepared_node(
                        node,
                        &prepared,
                        Some(&m_deps),
                        yes,
                        true,
                        true,
                        verbose
                    ) {
                        Ok(manifest) => {
                            dependency_installed_count
                                .fetch_add(1, Ordering::Relaxed);
                            let mut tx_lock =
                                transaction.lock().map_err(|e| {
                                    anyhow!(
                                        "Transaction mutex poisoned during \
                                         installation: {e}"
                                    )
                                })?;
                            if let Err(e) = transaction::record_operation(
                                &mut tx_lock,
                                types::TransactionOperation::Install {
                                    manifest: Box::new(manifest)
                                }
                            ) {
                                return Err(anyhow!(
                                    "Transaction recording failed: {e}"
                                ));
                            }
                        }
                        Err(e) => {
                            failed_packages
                                .lock()
                                .expect("Failed packages mutex poisoned")
                                .push(node.pkg.name.clone());
                            eprintln!(
                                "Error installing {}: {}",
                                node.pkg.name, e
                            );
                        }
                    }
                    Ok(())
                })?;
            }
        }

        println!("\n{} Installing packages...", "::".bold().blue());
        let m_pkg = MultiProgress::new();

        for stage in &stages {
            let mut stage_direct_ids = Vec::new();
            for pkg_id in stage {
                if let Some(node) = graph.nodes.get(pkg_id)
                    && matches!(node.reason, types::InstallReason::Direct)
                {
                    let name = if let Some(sub) = &node.sub_package {
                        format!("{}:{}", node.pkg.name, sub)
                    } else {
                        node.pkg.name.clone()
                    };
                    let version_display = if node.revision == "1" {
                        node.version.clone()
                    } else {
                        format!("{}-{}", node.version, node.revision)
                    };
                    println!("@{name}:{version_display}");
                    stage_direct_ids.push(pkg_id.clone());
                }
            }

            if stage_direct_ids.is_empty() {
                continue;
            }

            let res = stage_direct_ids.par_iter().try_for_each(
                |pkg_id| -> Result<()> {
                    let node = graph.nodes.get(pkg_id).ok_or_else(|| {
                        anyhow!(
                            "Package node '{pkg_id}' missing from graph \
                             during final installation"
                        )
                    })?;

                    let prepared = {
                        let lock = prepared_nodes.lock().map_err(|e| {
                            anyhow!(
                                "Prepared nodes mutex poisoned during package \
                                 install: {e}"
                            )
                        })?;
                        lock.get(pkg_id).cloned().ok_or_else(|| {
                            anyhow!("Prepared node missing for: {pkg_id}")
                        })?
                    };

                    match installer::install_prepared_node(
                        node,
                        &prepared,
                        Some(&m_pkg),
                        yes,
                        true,
                        true,
                        verbose
                    ) {
                        Ok(manifest) => {
                            installed_manifests
                                .lock()
                                .expect("Installed manifests mutex poisoned")
                                .push(manifest.clone());
                            let mut tx_lock =
                                transaction.lock().map_err(|e| {
                                    anyhow!(
                                        "Transaction mutex poisoned during \
                                         direct package installation: {e}"
                                    )
                                })?;
                            transaction::record_operation(
                                &mut tx_lock,
                                types::TransactionOperation::Install {
                                    manifest: Box::new(manifest)
                                }
                            )?;
                            successfully_installed_sources
                                .lock()
                                .expect(
                                    "Successfully installed sources mutex \
                                     poisoned"
                                )
                                .push(node.source.clone());
                            Ok(())
                        }
                        Err(e) => {
                            failed_packages
                                .lock()
                                .expect("Failed packages mutex poisoned")
                                .push(node.pkg.name.clone());
                            eprintln!(
                                "Error installing {}: {}",
                                node.pkg.name, e
                            );
                            Err(e)
                        }
                    }
                }
            );

            if res.is_err() {
                break;
            }
        }

        let failed = failed_packages
            .lock()
            .expect("Failed packages mutex poisoned");
        if !failed.is_empty() {
            println!("\n{} Rolling back changes...", "::".bold().yellow());
            transaction::rollback(&transaction_id)?;
            return Err(anyhow!(
                "Installation failed for: {}",
                failed.join(", ")
            ));
        }

        if let Err(e) = transaction::commit(&transaction_id) {
            eprintln!("Warning: Failed to commit transaction: {e}");
        }

        let installed_manifests_vec = installed_manifests
            .lock()
            .expect("Installed manifests mutex poisoned")
            .clone();
        for manifest in &installed_manifests_vec {
            if let Some(pm) = options.plugin_manager {
                let pkg_val = pm
                    .lua
                    .to_value(manifest)
                    .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
                pm.trigger_hook_nonfatal("on_post_install", Some(&pkg_val));
            }
        }

        println!("\n{} Installation complete!", "Success:".green().bold());
        Ok(())
    }
}
