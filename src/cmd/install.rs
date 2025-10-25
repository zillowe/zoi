use crate::pkg::{config, install, transaction, types};
use crate::project;
use colored::Colorize;
use rayon::prelude::*;
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
) {
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

    let lockfile_exists = sources.is_empty()
        && repo.is_none()
        && std::path::Path::new("zoi.lock").exists()
        && std::path::Path::new("zoi.yaml").exists();

    let mut sources_to_process: Vec<String> = sources.to_vec();
    let mut is_project_install = false;
    if sources.is_empty()
        && repo.is_none()
        && let Ok(config) = project::config::load()
        && config.config.local
    {
        if lockfile_exists {
            println!("zoi.lock found. Installing from zoi.yaml then verifying...");
        } else {
            println!("Installing project packages from zoi.yaml...");
        }
        sources_to_process = config.pkgs.clone();
        scope_override = Some(types::Scope::Project);
        is_project_install = true;
    }

    if sources_to_process.is_empty() {
        return;
    }

    if let Some(repo_spec) = repo {
        if scope_override == Some(types::Scope::Project) {
            eprintln!(
                "{}: Installing from a repository to a project scope is not supported.",
                "Error".red().bold()
            );
            std::process::exit(1);
        }
        let repo_install_scope = scope_override.map(|s| match s {
            types::Scope::User => crate::cli::SetupScope::User,
            types::Scope::System => crate::cli::SetupScope::System,
            types::Scope::Project => unreachable!(),
        });

        if let Err(e) =
            crate::pkg::repo_install::run(&repo_spec, force, all_optional, yes, repo_install_scope)
        {
            eprintln!(
                "{}: Failed to install from repo '{}': {}",
                "Error".red().bold(),
                repo_spec,
                e
            );
            std::process::exit(1);
        }
        return;
    }

    let config = config::read_config().unwrap_or_default();
    let parallel_jobs = config.parallel_jobs.unwrap_or(3);
    if parallel_jobs > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(parallel_jobs)
            .build_global()
            .unwrap();
    }

    let mode = install::InstallMode::PreferPrebuilt;
    let failed_packages = Mutex::new(Vec::new());
    let mut temp_files = Vec::new();
    let mut final_sources = Vec::new();

    for source in &sources_to_process {
        if source.ends_with("zoi.pkgs.json") {
            if let Err(e) =
                install::lockfile::process_lockfile(source, &mut final_sources, &mut temp_files)
            {
                eprintln!(
                    "{}: Failed to process lockfile '{}': {}",
                    "Error".red().bold(),
                    source,
                    e
                );
                failed_packages.lock().unwrap().push(source.to_string());
            }
        } else {
            final_sources.push(source.to_string());
        }
    }

    let successfully_installed_sources = Mutex::new(Vec::new());
    let installed_manifests = Mutex::new(Vec::new());

    println!("{}", "Resolving dependencies...".bold());

    let graph = match install::resolver::resolve_dependency_graph(
        &final_sources,
        scope_override,
        force,
        yes,
        all_optional,
    ) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{}: {}", "Failed to resolve dependencies".red().bold(), e);
            std::process::exit(1);
        }
    };

    let stages = match graph.toposort() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", "Failed to sort dependencies".red().bold(), e);
            std::process::exit(1);
        }
    };

    let transaction = match transaction::begin() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to begin transaction: {}", e);
            std::process::exit(1);
        }
    };

    println!("\nStarting installation...");
    let mut overall_success = true;

    for (i, stage) in stages.iter().enumerate() {
        println!(
            "--- Installing Stage {}/{} ({} packages)",
            i + 1,
            stages.len(),
            stage.len()
        );

        stage.par_iter().for_each(|pkg_id| {
            let node = graph.nodes.get(pkg_id).unwrap();
            println!("Installing {}...", node.pkg.name.cyan());

            match install::installer::install_node(node, mode, None, build_type.as_deref(), yes) {
                Ok(manifest) => {
                    println!("Successfully installed {}", node.pkg.name.green());
                    installed_manifests.lock().unwrap().push(manifest.clone());

                    if let Err(e) = transaction::record_operation(
                        &transaction.id,
                        types::TransactionOperation::Install {
                            manifest: Box::new(manifest),
                        },
                    ) {
                        eprintln!(
                            "Error: Failed to record transaction operation for {}: {}",
                            node.pkg.name, e
                        );
                        failed_packages.lock().unwrap().push(node.pkg.name.clone());
                    }

                    if matches!(node.reason, types::InstallReason::Direct) {
                        successfully_installed_sources
                            .lock()
                            .unwrap()
                            .push(node.source.clone());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{}: Failed to install {}: {}",
                        "Error".red().bold(),
                        node.pkg.name,
                        e
                    );
                    failed_packages.lock().unwrap().push(node.pkg.name.clone());
                }
            }
        });

        let failed = failed_packages.lock().unwrap();
        if !failed.is_empty() {
            eprintln!(
                "\n{}: Installation failed at stage {}.",
                "Error".red().bold(),
                i + 1
            );
            overall_success = false;
            break;
        }
    }

    if !overall_success {
        eprintln!(
            "\n{}: The following packages failed to install:",
            "Error".red().bold()
        );
        for pkg in &failed_packages.into_inner().unwrap() {
            eprintln!("  - {}", pkg);
        }

        eprintln!("\n{} Rolling back changes...", "---".yellow().bold());
        if let Err(e) = transaction::rollback(&transaction.id) {
            eprintln!("\nCRITICAL: Rollback failed: {}", e);
            eprintln!(
                "The system may be in an inconsistent state. The transaction log is at ~/.zoi/transactions/{}.json",
                transaction.id
            );
        } else {
            println!("\n{} Rollback successful.", "Success:".green().bold());
        }

        std::process::exit(1);
    }

    if let Err(e) = transaction::commit(&transaction.id) {
        eprintln!("Warning: Failed to commit transaction: {}", e);
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

            let installed_manifests = installed_manifests.into_inner().unwrap();
            for manifest in &installed_manifests {
                let full_id = format!(
                    "#{}@{}/{}",
                    manifest.registry_handle, manifest.repo, manifest.name
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
                )
                .unwrap();
                let latest_dir = package_dir.join("latest");
                let integrity =
                    crate::pkg::hash::calculate_dir_hash(&latest_dir).unwrap_or_else(|e| {
                        eprintln!(
                            "Warning: could not calculate integrity for {}: {}",
                            manifest.name, e
                        );
                        String::new()
                    });

                let pkg_id = format!("{}@{}", manifest.name, manifest.version);
                let dependencies: Vec<String> = graph
                    .adj
                    .get(&pkg_id)
                    .map(|deps| {
                        deps.iter()
                            .map(|dep_id| {
                                let node = graph.nodes.get(dep_id).unwrap();
                                format!(
                                    "#{}@{}/{}",
                                    node.registry_handle, node.pkg.repo, node.pkg.name
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let detail = types::LockPackageDetail {
                    version: manifest.version.clone(),
                    integrity,
                    dependencies,
                    options_dependencies: manifest.chosen_options.clone(),
                    optionals_dependencies: manifest.chosen_optionals.clone(),
                };

                let registry_key = format!("#{}", manifest.registry_handle);
                let short_id = format!("@{}/{}", manifest.repo, manifest.name);

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
        let successfully_installed = successfully_installed_sources.into_inner().unwrap();
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
        if let Err(e) = project::verify::run() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            eprintln!(
                "\nzoi.lock is out of sync with zoi.yaml. Run `rm zoi.lock && zoi install` to update it."
            );
            std::process::exit(1);
        }
    }
}
