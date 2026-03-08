use crate::pkg::{config, install, lock, resolve, transaction, types};
use crate::project;
use anyhow::{Result, anyhow};
use colored::Colorize;
use indicatif::MultiProgress;
use mlua::LuaSerdeExt;
use rayon::prelude::*;
use std::collections::HashSet;
use std::sync::Mutex;

pub fn run(
    sources: &[String],
    repo: Option<String>,
    force: bool,
    all_optional: bool,
    yes: bool,
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
    save: bool,
    build_type: Option<String>,
    dry_run: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
    build: bool,
) -> Result<()> {
    let mut scope_override = scope.map(|s| match s {
        crate::cli::InstallScope::User => types::Scope::User,
        crate::cli::InstallScope::System => types::Scope::System,
        crate::cli::InstallScope::Project => types::Scope::Project,
    });

    if local {
        scope_override = Some(types::Scope::Project);
    } else if global {
        scope_override = Some(types::Scope::User);
    }

    if save && scope_override.is_none() && std::path::Path::new("zoi.yaml").exists() {
        scope_override = Some(types::Scope::Project);
    }

    let lockfile_exists = sources.is_empty()
        && repo.is_none()
        && std::path::Path::new("zoi.lock").exists()
        && std::path::Path::new("zoi.yaml").exists();

    let mut sources_to_process: Vec<String> = sources.to_vec();
    let mut is_project_install = false;
    if sources.is_empty()
        && repo.is_none()
        && let Ok(config) = project::config::load()
    {
        if lockfile_exists {
            println!(
                "{} zoi.lock found. Installing from zoi.yaml then verifying...",
                "::".bold().blue()
            );
        } else {
            println!(
                "{} Installing project packages from zoi.yaml...",
                "::".bold().blue()
            );
        }
        sources_to_process = config.pkgs.clone();
        if scope_override.is_none() {
            scope_override = Some(types::Scope::Project);
        }
        is_project_install = true;
    }

    if let Some(repo_spec) = repo {
        if scope_override == Some(types::Scope::Project) {
            return Err(anyhow!(
                "Installing from a repository to a project scope is not supported."
            ));
        }
        let repo_install_scope = scope_override.map(|s| match s {
            types::Scope::User => crate::cli::SetupScope::User,
            types::Scope::System => crate::cli::SetupScope::System,
            types::Scope::Project => unreachable!(),
        });

        if dry_run {
            println!(
                "{} Dry-run: would install from repository '{}'",
                "::".bold().yellow(),
                repo_spec
            );
            return Ok(());
        }

        crate::pkg::repo_install::run(
            &repo_spec,
            force,
            all_optional,
            yes,
            repo_install_scope,
            plugin_manager,
        )?;
        return Ok(());
    }

    if sources_to_process.is_empty() {
        return Ok(());
    }

    let config = config::read_config().unwrap_or_default();
    let parallel_jobs = config.parallel_jobs.unwrap_or(3);
    if parallel_jobs > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_jobs)
            .build_global()
            .ok();
    }

    let failed_packages = Mutex::new(Vec::new());
    let mut temp_files = Vec::new();
    let mut final_sources = Vec::new();

    for source in &sources_to_process {
        if source.ends_with("zoi.pkgs.json") {
            install::lockfile::process_lockfile(source, &mut final_sources, &mut temp_files)?;
        } else {
            final_sources.push(source.to_string());
        }
    }

    let successfully_installed_sources = Mutex::new(Vec::new());
    let installed_manifests = Mutex::new(Vec::new());

    let (mut graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &final_sources,
        scope_override,
        force,
        yes,
        all_optional,
        build_type.as_deref(),
        false,
    )?;

    if !force {
        let mut to_remove = Vec::new();
        for (pkg_id, node) in &graph.nodes {
            let request = resolve::parse_source_string(&node.source)?;
            if let Some(manifest) = crate::pkg::local::is_package_installed(
                &node.pkg.name,
                request.sub_package.as_deref(),
                scope_override.unwrap_or(node.pkg.scope),
            )? && manifest.version == node.version
            {
                println!(
                    "{} Package '{}' is already installed at version {}. Skipping.",
                    "::".bold().green(),
                    node.pkg.name.cyan(),
                    node.version.yellow()
                );
                to_remove.push(pkg_id.clone());
            }
        }

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
    }

    if graph.nodes.is_empty() && non_zoi_deps.is_empty() {
        println!("\nAll requested packages are already installed.");
        return Ok(());
    }

    if !dry_run {
        for node in graph.nodes.values() {
            let pkg_val = plugin_manager
                .lua
                .to_value(&node.pkg)
                .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
            plugin_manager.trigger_hook("on_pre_install", Some(pkg_val))?;
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

    println!("{} Looking for conflicts...", "::".bold().blue());
    let packages_to_install: Vec<&types::Package> = graph.nodes.values().map(|n| &n.pkg).collect();

    if !dry_run {
        install::util::check_for_conflicts(&packages_to_install, yes)?;

        let m_for_conflict_check = MultiProgress::new();
        install::util::check_file_conflicts(&graph, yes, &m_for_conflict_check)?;
        let _ = m_for_conflict_check.clear();
    }

    println!("{} Checking available disk space...", "::".bold().blue());
    let install_plan =
        install::plan::create_install_plan(&graph.nodes, build_type.as_deref(), build)?;

    let mut total_download_size: u64 = 0;
    let mut total_installed_size: u64 = 0;
    let mut unique_downloads = HashSet::new();

    for (id, node) in &graph.nodes {
        match install_plan.get(id) {
            Some(install::plan::InstallAction::DownloadAndInstall(details)) => {
                if unique_downloads.insert(details.info.final_url.clone()) {
                    total_download_size += details.download_size;
                }
                total_installed_size += if details.installed_size > 0 {
                    details.installed_size
                } else {
                    node.pkg.installed_size.unwrap_or(0)
                };
            }
            Some(install::plan::InstallAction::BuildAndInstall) => {
                total_installed_size += node.pkg.installed_size.unwrap_or(0);
            }
            _ => {}
        }
    }

    println!(
        "\n{} Packages ({})",
        "::".bold().blue(),
        direct_packages.len()
    );
    let direct_list: Vec<_> = direct_packages
        .iter()
        .map(|n| {
            let name = if let Some(sub) = &n.sub_package {
                format!("{}:{}", n.pkg.name, sub)
            } else {
                n.pkg.name.clone()
            };
            format!("@{}:{}", name, n.version).cyan().to_string()
        })
        .collect();
    println!(" {}", direct_list.join("  "));

    if !dependencies.is_empty() || !non_zoi_deps.is_empty() {
        println!(
            "\n{} Dependencies ({})",
            "::".bold().blue(),
            dependencies.len() + non_zoi_deps.len()
        );
        let mut dep_list = Vec::new();
        for n in &dependencies {
            let name = if let Some(sub) = &n.sub_package {
                format!("{}:{}", n.pkg.name, sub)
            } else {
                n.pkg.name.clone()
            };
            dep_list.push(format!("zoi: @{}:{}", name, n.version).dimmed().to_string());
        }
        for d in &non_zoi_deps {
            dep_list.push(d.dimmed().to_string());
        }
        println!(" {}", dep_list.join("  "));
    }

    if total_download_size > 0 {
        println!(
            "\nTotal Download Size:  {}",
            crate::utils::format_bytes(total_download_size)
        );
    }
    if total_installed_size > 0 {
        println!(
            "Total Installed Size: {}",
            crate::utils::format_bytes(total_installed_size)
        );
    }

    if dry_run {
        println!(
            "\n{} Dry-run: installation plan above would be executed.",
            "::".bold().yellow()
        );
        return Ok(());
    }

    let install_path = crate::pkg::local::get_store_base_dir(scope_override.unwrap_or_default())?;
    std::fs::create_dir_all(&install_path)?;

    let available_space = fs2::available_space(&install_path).unwrap_or(u64::MAX);

    if total_installed_size > available_space {
        return Err(anyhow!(
            "Not enough disk space. Required: {}, Available: {}",
            crate::utils::format_bytes(total_installed_size),
            crate::utils::format_bytes(available_space)
        ));
    }

    if !crate::utils::ask_for_confirmation("\nProceed with installation?", yes) {
        let _ = lock::release_lock();
        return Ok(());
    }

    let stages = graph.toposort()?;
    let transaction = transaction::begin()?;
    let transaction_id = &transaction.id;
    let transaction_mutex = Mutex::new(());

    if !dependencies.is_empty() || !non_zoi_deps.is_empty() {
        println!("\n{} Installing dependencies...", "::".bold().blue());
        let m_deps = MultiProgress::new();

        if !non_zoi_deps.is_empty() {
            let processed_deps = Mutex::new(HashSet::new());
            let mut installed_deps_ext = Vec::new();
            for dep_str in &non_zoi_deps {
                let dep = match crate::pkg::dependencies::parse_dependency_string(dep_str) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error parsing dependency {}: {}", dep_str, e);
                        continue;
                    }
                };

                if let Err(e) = crate::pkg::dependencies::install_dependency(
                    &dep,
                    "direct",
                    scope_override.unwrap_or_default(),
                    yes,
                    all_optional,
                    &processed_deps,
                    &mut installed_deps_ext,
                    Some(&m_deps),
                ) {
                    eprintln!("Failed to install dependency {}: {}", dep_str, e);
                }
            }
        }

        for stage in &stages {
            stage.par_iter().for_each(|pkg_id| {
                let node = graph
                    .nodes
                    .get(pkg_id)
                    .expect("Package node missing from graph during installation");
                if matches!(node.reason, types::InstallReason::Direct) {
                    return;
                }
                let action = install_plan
                    .get(pkg_id)
                    .expect("Install action missing for package during installation");

                match install::installer::install_node(
                    node,
                    action,
                    Some(&m_deps),
                    build_type.as_deref(),
                    yes,
                    true,
                ) {
                    Ok(manifest) => {
                        let _lock = transaction_mutex
                            .lock()
                            .expect("Transaction mutex poisoned during installation");
                        let _ = transaction::record_operation(
                            transaction_id,
                            types::TransactionOperation::Install {
                                manifest: Box::new(manifest),
                            },
                        );
                    }
                    Err(e) => {
                        failed_packages
                            .lock()
                            .expect("Failed packages mutex poisoned")
                            .push(node.pkg.name.clone());
                        eprintln!("Error installing {}: {}", node.pkg.name, e);
                    }
                }
            });
        }
    }

    println!("\n{} Installing packages...", "::".bold().blue());
    for stage in &stages {
        for pkg_id in stage {
            let node = graph
                .nodes
                .get(pkg_id)
                .expect("Package node missing from graph during final installation");
            if !matches!(node.reason, types::InstallReason::Direct) {
                continue;
            }
            let name = if let Some(sub) = &node.sub_package {
                format!("{}:{}", node.pkg.name, sub)
            } else {
                node.pkg.name.clone()
            };
            println!(" @{}:{}", name, node.version);

            let action = install_plan
                .get(pkg_id)
                .expect("Install action missing for package during final installation");
            let m_pkg = MultiProgress::new();

            match install::installer::install_node(
                node,
                action,
                Some(&m_pkg),
                build_type.as_deref(),
                yes,
                true,
            ) {
                Ok(manifest) => {
                    installed_manifests
                        .lock()
                        .expect("Installed manifests mutex poisoned")
                        .push(manifest.clone());
                    let _ = transaction::record_operation(
                        transaction_id,
                        types::TransactionOperation::Install {
                            manifest: Box::new(manifest),
                        },
                    );
                    successfully_installed_sources
                        .lock()
                        .expect("Successfully installed sources mutex poisoned")
                        .push(node.source.clone());
                }
                Err(e) => {
                    failed_packages
                        .lock()
                        .expect("Failed packages mutex poisoned")
                        .push(node.pkg.name.clone());
                    eprintln!("Error: {}", e);
                }
            }
        }
    }

    let failed = failed_packages
        .lock()
        .expect("Failed packages mutex poisoned");
    if !failed.is_empty() {
        println!("\n{} Rolling back changes...", "::".bold().yellow());
        transaction::rollback(&transaction.id)?;
        return Err(anyhow!("Installation failed for: {}", failed.join(", ")));
    }

    if let Ok(modified_files) = transaction::get_modified_files(&transaction.id) {
        let _ = crate::pkg::hooks::global::run_global_hooks(
            crate::pkg::hooks::global::HookWhen::PostTransaction,
            &modified_files,
            "install",
        );
    }

    if let Err(e) = transaction::commit(&transaction.id) {
        eprintln!("Warning: Failed to commit transaction: {}", e);
    }

    let installed_manifests_vec = installed_manifests
        .lock()
        .expect("Installed manifests mutex poisoned during finalization")
        .clone();
    for manifest in &installed_manifests_vec {
        let pkg_val = plugin_manager
            .lua
            .to_value(manifest)
            .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
        plugin_manager.trigger_hook("on_post_install", Some(pkg_val))?;
    }

    let is_any_project_install = scope_override == Some(types::Scope::Project);

    if is_any_project_install {
        if is_project_install && lockfile_exists {
        } else {
            println!("\nUpdating zoi.lock...");
            let mut lockfile =
                project::lockfile::read_zoi_lock().unwrap_or_else(|_| types::ZoiLock {
                    version: "1".to_string(),
                    ..Default::default()
                });

            lockfile.packages.clear();
            lockfile.details.clear();

            let all_regs_config = crate::pkg::config::read_config().unwrap_or_default();
            let mut all_configured_regs = all_regs_config.added_registries;
            if let Some(default_reg) = all_regs_config.default_registry {
                all_configured_regs.push(default_reg);
            }

            let installed_manifests = installed_manifests
                .into_inner()
                .expect("Installed manifests mutex poisoned during lockfile update");
            for manifest in &installed_manifests {
                let name_with_sub = if let Some(sub) = &manifest.sub_package {
                    format!("{}:{}", manifest.name, sub)
                } else {
                    manifest.name.clone()
                };

                let full_id = format!(
                    "#{}@{}/{}",
                    manifest.registry_handle, manifest.repo, name_with_sub
                );
                lockfile.packages.insert(full_id, manifest.version.clone());

                if let Some(reg) = all_configured_regs
                    .iter()
                    .find(|r| r.handle == manifest.registry_handle)
                {
                    lockfile
                        .registries
                        .insert(reg.handle.clone(), reg.url.clone());
                }

                let package_dir = crate::pkg::local::get_package_dir(
                    types::Scope::Project,
                    &manifest.registry_handle,
                    &manifest.repo,
                    &manifest.name,
                )?;
                let latest_dir = package_dir.join("latest");
                let integrity =
                    crate::pkg::hash::calculate_dir_hash(&latest_dir).unwrap_or_else(|e| {
                        eprintln!(
                            "Warning: could not calculate integrity for {}: {}",
                            manifest.name, e
                        );
                        String::new()
                    });

                let pkg_id = if let Some(sub) = &manifest.sub_package {
                    format!("{}@{}:{}", manifest.name, manifest.version, sub)
                } else {
                    format!("{}@{}", manifest.name, manifest.version)
                };

                let dependencies: Vec<String> = graph
                    .adj
                    .get(&pkg_id)
                    .map(|deps| {
                        deps.iter()
                            .map(|dep_id| {
                                let node = graph.nodes.get(dep_id).expect(
                                    "Dependency node missing from graph during lockfile update",
                                );
                                if let Some(sub) = &node.pkg.sub_packages {
                                    format!(
                                        "#{}@{}/{}:{}",
                                        node.registry_handle,
                                        node.pkg.repo,
                                        node.pkg.name,
                                        sub.join(",")
                                    )
                                } else {
                                    format!(
                                        "#{}@{}/{}",
                                        node.registry_handle, node.pkg.repo, node.pkg.name
                                    )
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let detail = types::LockPackageDetail {
                    version: manifest.version.clone(),
                    sub_package: manifest.sub_package.clone(),
                    integrity,
                    dependencies,
                    options_dependencies: manifest.chosen_options.clone(),
                    optionals_dependencies: manifest.chosen_optionals.clone(),
                };

                let registry_key = format!("#{}", manifest.registry_handle);
                let short_id = format!("@{}/{}", manifest.repo, name_with_sub);

                lockfile
                    .details
                    .entry(registry_key)
                    .or_default()
                    .insert(short_id, detail);
            }

            if let Err(e) = project::lockfile::write_zoi_lock(&lockfile) {
                eprintln!("Warning: Failed to write zoi.lock file: {}", e);
            }
        }
    }

    if save && scope_override == Some(types::Scope::Project) {
        let successfully_installed = successfully_installed_sources
            .into_inner()
            .expect("Successfully installed sources mutex poisoned during finalization");
        if !successfully_installed.is_empty()
            && let Err(e) = project::config::add_packages_to_config(&successfully_installed)
        {
            eprintln!(
                "{}: Failed to save packages to zoi.yaml: {}",
                "Warning".yellow().bold(),
                e
            );
        }
    }

    println!("\n{} Installation complete!", "Success:".green().bold());

    if is_project_install && lockfile_exists {
        println!();
        project::verify::run()?;
    }

    println!("\n{} Done", "::".bold().blue());
    println!(
        "Installed ({}) packages and ({}) dependencies.",
        direct_packages.len(),
        dependencies.len() + non_zoi_deps.len()
    );

    Ok(())
}
