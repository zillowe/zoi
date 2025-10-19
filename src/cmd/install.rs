use crate::pkg::{config, install, resolve, transaction, types};
use crate::project;
use colored::Colorize;
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

    if sources.is_empty()
        && repo.is_none()
        && let Ok(config) = project::config::load()
        && config.config.local
    {
        let old_lockfile = project::lockfile::read_zoi_lock().ok();

        println!("Installing project packages locally...");
        let local_scope = Some(types::Scope::Project);
        let failed_packages = Mutex::new(Vec::new());
        let processed_deps = Mutex::new(HashSet::new());
        let installed_packages_info = Mutex::new(Vec::new());

        config.pkgs.par_iter().for_each(|source| {
            println!("=> Installing package: {}", source.cyan().bold());
            if let Err(e) = install::run_installation(
                source,
                install::InstallMode::PreferPrebuilt,
                force,
                types::InstallReason::Direct,
                yes,
                all_optional,
                &processed_deps,
                local_scope,
                None,
            ) {
                eprintln!(
                    "{}: Failed to install '{}': {}",
                    "Error".red().bold(),
                    source,
                    e
                );
                failed_packages.lock().unwrap().push(source.to_string());
            } else if let Ok((pkg, _, _, _, registry_handle)) =
                resolve::resolve_package_and_version(source)
            {
                installed_packages_info
                    .lock()
                    .unwrap()
                    .push((pkg, registry_handle));
            }
        });

        let failed_packages = failed_packages.into_inner().unwrap();
        if !failed_packages.is_empty() {
            eprintln!(
                "\n{}: The following packages failed to install:",
                "Error".red().bold()
            );
            for pkg in &failed_packages {
                eprintln!("  - {}", pkg);
            }
            std::process::exit(1);
        }

        let mut new_lockfile_packages = std::collections::HashMap::new();
        let installed_packages_info = installed_packages_info.into_inner().unwrap();
        for (pkg, registry_handle) in &installed_packages_info {
            let handle = registry_handle.as_deref().unwrap_or("local");
            if let Ok(package_dir) = crate::pkg::local::get_package_dir(
                types::Scope::Project,
                handle,
                &pkg.repo,
                &pkg.name,
            ) {
                let latest_dir = package_dir.join("latest");
                if let Ok(hash) = crate::pkg::hash::calculate_dir_hash(&latest_dir) {
                    new_lockfile_packages.insert(pkg.name.clone(), hash);
                }
            }
        }

        if let Some(old_lock) = old_lockfile {
            for (pkg_name, new_hash) in &new_lockfile_packages {
                if let Some(old_hash) = old_lock.packages.get(pkg_name)
                    && old_hash != new_hash
                {
                    println!("Warning: Hash mismatch for package '{}'.", pkg_name);
                }
            }
        }

        let new_lockfile = types::ZoiLock {
            packages: new_lockfile_packages,
        };
        if let Err(e) = project::lockfile::write_zoi_lock(&new_lockfile) {
            eprintln!("Warning: Failed to write zoi.lock file: {}", e);
        }

        return;
    }

    if scope_override.is_none()
        && let Ok(config) = project::config::load()
        && config.config.local
    {
        scope_override = Some(types::Scope::Project);
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

    let mut sources_to_process: Vec<String> = Vec::new();

    for source in sources {
        if source.ends_with("zoi.pkgs.json") {
            if let Err(e) = install::lockfile::process_lockfile(
                source,
                &mut sources_to_process,
                &mut temp_files,
            ) {
                eprintln!(
                    "{}: Failed to process lockfile '{}': {}",
                    "Error".red().bold(),
                    source,
                    e
                );

                failed_packages.lock().unwrap().push(source.to_string());
            }
        } else {
            sources_to_process.push(source.to_string());
        }
    }

    let successfully_installed_sources = Mutex::new(Vec::new());

    println!("{}", "Resolving dependencies...".bold());

    let graph = match install::resolver::resolve_dependency_graph(
        &sources_to_process,
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
            "--- Installing Stage {}/{} ({} packages) ---",
            i + 1,
            stages.len(),
            stage.len()
        );

        stage.par_iter().for_each(|pkg_id| {
            let node = graph.nodes.get(pkg_id).unwrap();

            println!("Installing {}...", node.pkg.name.cyan());

            match install::installer::install_node(node, mode, None) {
                Ok(manifest) => {
                    println!("Successfully installed {}", node.pkg.name.green());

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
}
