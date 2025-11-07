use crate::pkg::{self, config, install, local, lock, transaction, types};
use crate::project;
use colored::Colorize;
use indicatif::MultiProgress;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
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

    let (graph, non_zoi_deps) = match install::resolver::resolve_dependency_graph(
        &final_sources,
        scope_override,
        force,
        yes,
        all_optional,
        build_type.as_deref(),
        true,
    ) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("{}: {}", "Failed to resolve dependencies".red().bold(), e);
            std::process::exit(1);
        }
    };

    let packages_to_install: Vec<&types::Package> = graph.nodes.values().map(|n| &n.pkg).collect();

    let mut packages_to_replace = std::collections::HashSet::new();
    if let Ok(installed_packages) = local::get_installed_packages() {
        for pkg in &packages_to_install {
            if let Some(replaces) = &pkg.replaces {
                for replaced_pkg_name in replaces {
                    if installed_packages
                        .iter()
                        .any(|p| &p.name == replaced_pkg_name)
                    {
                        packages_to_replace.insert(replaced_pkg_name.clone());
                    }
                }
            }
        }
    }

    if !packages_to_replace.is_empty() {
        println!("\nThe following packages will be replaced:");
        for pkg_name in &packages_to_replace {
            println!("- {}", pkg_name);
        }
        if !crate::utils::ask_for_confirmation(
            "\nDo you want to continue with the replacement?",
            yes,
        ) {
            return;
        }
    }

    println!("{}", "Looking for conflicts...".bold());
    if let Err(e) = install::util::check_for_conflicts(&packages_to_install, yes) {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }

    let m_for_conflict_check = MultiProgress::new();
    if let Err(e) = install::util::check_file_conflicts(&graph, yes, &m_for_conflict_check) {
        eprintln!("{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
    let _ = m_for_conflict_check.clear();

    let install_plan = match install::plan::create_install_plan(&graph.nodes) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("{}: {}", "Failed to create install plan".red().bold(), e);
            std::process::exit(1);
        }
    };

    let mut to_download = HashMap::new();
    let mut to_build = HashMap::new();

    for (id, node) in &graph.nodes {
        match install_plan.get(id) {
            Some(install::plan::InstallAction::DownloadAndInstall(details)) => {
                to_download.insert(id.clone(), (node, details.clone()));
            }
            Some(install::plan::InstallAction::BuildAndInstall) => {
                to_build.insert(id.clone(), node);
            }
            _ => {}
        }
    }

    let total_download_size: u64 = to_download.values().map(|(_, d)| d.download_size).sum();
    let total_installed_size: u64 = to_download
        .values()
        .map(|(n, _)| n.pkg.installed_size.unwrap_or(0))
        .sum();

    println!("\n--- Summary ---");

    if !to_download.is_empty() {
        println!("\nPackages to download:");
        let pkg_list: Vec<_> = to_download
            .values()
            .map(|(n, _)| {
                if let Some(sub) = &n.sub_package {
                    format!("{}:{}@{}", n.pkg.name, sub, n.version)
                } else {
                    format!("{}@{}", n.pkg.name, n.version)
                }
            })
            .collect();
        println!("{}", pkg_list.join(" "));
        println!(
            "Total Download Size:  {}",
            crate::utils::format_bytes(total_download_size)
        );
        println!(
            "Total Installed Size: {}",
            crate::utils::format_bytes(total_installed_size)
        );
    }

    if !to_build.is_empty() {
        println!("Packages to build from source:");
        let pkg_list: Vec<_> = to_build
            .values()
            .map(|n| {
                if let Some(sub) = &n.sub_package {
                    format!("{}:{}@{}", n.pkg.name, sub, n.version)
                } else {
                    format!("{}@{}", n.pkg.name, n.version)
                }
            })
            .collect();
        println!("{}", pkg_list.join(" "));
        println!(
            "{}",
            "(Sizes for built packages are not available beforehand)".yellow()
        );
    }

    if !non_zoi_deps.is_empty() {
        println!("\nExternal dependencies:");
        let pkg_list: Vec<_> = non_zoi_deps.iter().map(|d| d.cyan().to_string()).collect();
        println!("{}", pkg_list.join(" "));
    }

    if !to_build.is_empty() {
        println!(
            "\n{}: Disk space check is skipped when packages need to be built from source.",
            "Warning".yellow()
        );
    } else {
        println!("\n{}", "Checking available disk space...".bold());
        let install_path =
            match crate::pkg::local::get_store_base_dir(scope_override.unwrap_or_default()) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!(
                        "{}: Could not determine install path: {}",
                        "Error".red().bold(),
                        e
                    );
                    std::process::exit(1);
                }
            };

        if let Err(e) = std::fs::create_dir_all(&install_path) {
            eprintln!(
                "{}: Could not create install directory: {}",
                "Error".red().bold(),
                e
            );
            std::process::exit(1);
        }

        let available_space = match fs2::available_space(&install_path) {
            Ok(space) => space,
            Err(e) => {
                eprintln!(
                    "{}: Could not check available disk space: {}",
                    "Warning".yellow().bold(),
                    e
                );
                u64::MAX
            }
        };

        if total_installed_size > available_space {
            eprintln!(
                "{}: Not enough disk space. Required: {}, Available: {}",
                "Error".red().bold(),
                crate::utils::format_bytes(total_installed_size),
                crate::utils::format_bytes(available_space)
            );
            std::process::exit(1);
        }
    }

    if !crate::utils::ask_for_confirmation("\n:: Proceed with installation?", yes) {
        let _ = lock::release_lock();
        return;
    }

    let mut final_install_plan = install_plan.clone();

    if !to_download.is_empty() {
        println!("\n:: Downloading packages...");
        let mut download_groups: HashMap<String, (&install::plan::PrebuiltDetails, Vec<&str>)> =
            HashMap::new();

        for (node, details) in to_download.values() {
            let entry = download_groups
                .entry(details.info.final_url.clone())
                .or_insert((details, Vec::new()));
            if let Some(sub) = &node.sub_package {
                entry.1.push(sub);
            }
        }

        let downloaded_archives: Mutex<HashMap<String, std::path::PathBuf>> =
            Mutex::new(HashMap::new());
        let m_for_dl = MultiProgress::new();

        download_groups.par_iter().for_each(|(url, (details, _))| {
            let first_node = to_download
                .values()
                .find(|(_, d)| d.info.final_url == *url)
                .unwrap()
                .0;

            match install::installer::download_and_cache_archive(
                first_node,
                details,
                Some(&m_for_dl),
            ) {
                Ok(path) => {
                    downloaded_archives
                        .lock()
                        .unwrap()
                        .insert(url.clone(), path);
                }
                Err(e) => {
                    eprintln!("Failed to download {}: {}", url, e);
                    failed_packages.lock().unwrap().push(url.clone());
                }
            }
        });

        let downloaded_archives_map = downloaded_archives.lock().unwrap();
        for (id, (_, details)) in &to_download {
            if let Some(downloaded_path) = downloaded_archives_map.get(&details.info.final_url) {
                final_install_plan.insert(
                    id.clone(),
                    install::plan::InstallAction::InstallFromArchive(downloaded_path.clone()),
                );
            }
        }
    }

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

    for pkg_name in packages_to_replace {
        println!("Replacing package: {}", pkg_name);
        match pkg::uninstall::run(&pkg_name, None) {
            Ok(uninstalled_manifest) => {
                if let Err(e) = transaction::record_operation(
                    &transaction.id,
                    types::TransactionOperation::Uninstall {
                        manifest: Box::new(uninstalled_manifest),
                    },
                ) {
                    eprintln!("Failed to record uninstall of replaced package: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to uninstall replaced package '{}': {}", pkg_name, e);
            }
        }
    }

    println!("\n:: Starting installation...");
    let mut overall_success = true;
    let m = MultiProgress::new();

    for (i, stage) in stages.iter().enumerate() {
        println!(
            ":: Installing Stage {}/{} ({} packages)",
            i + 1,
            stages.len(),
            stage.len()
        );

        stage.par_iter().for_each(|pkg_id| {
            let node = graph.nodes.get(pkg_id).unwrap();
            let action = final_install_plan.get(pkg_id).unwrap();

            match install::installer::install_node(
                node,
                action,
                Some(&m),
                build_type.as_deref(),
                yes,
            ) {
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

    if !non_zoi_deps.is_empty() {
        println!("\n:: Installing external dependencies...");
        let processed_deps = Mutex::new(HashSet::new());
        let mut installed_deps_ext = Vec::new();
        for dep_str in &non_zoi_deps {
            let dep = match crate::pkg::dependencies::parse_dependency_string(dep_str) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!(
                        "{}: Could not parse dependency string '{}': {}",
                        "Error".red().bold(),
                        dep_str,
                        e
                    );
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
                Some(&m),
            ) {
                eprintln!(
                    "{}: Failed to install external dependency {}: {}",
                    "Error".red().bold(),
                    dep_str,
                    e
                );
            }
        }
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
                                let node = graph.nodes.get(dep_id).unwrap();
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
