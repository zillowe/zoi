use crate::cmd::utils;
use crate::pkg::{self, lock, transaction, types};
use colored::*;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn run(
    package_names: &[String],
    scope: Option<crate::cli::InstallScope>,
    local: bool,
    global: bool,
    save: bool,
    yes: bool,
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

    if save && scope_override != Some(types::Scope::Project) {
        eprintln!(
            "{}: The --save flag can only be used with project-scoped uninstalls.",
            "Error".red().bold()
        );
        std::process::exit(1);
    }

    let installed_packages = match pkg::local::get_installed_packages() {
        Ok(pkgs) => pkgs,
        Err(e) => {
            eprintln!("Error reading installed packages: {}", e);
            std::process::exit(1);
        }
    };

    let mut manifests_to_uninstall: Vec<types::InstallManifest> = Vec::new();
    let mut failed_resolution = false;

    let expanded_names = match utils::expand_split_packages(package_names, "Uninstalling") {
        Ok(names) => names,
        Err(e) => {
            eprintln!("Error expanding packages: {}", e);
            std::process::exit(1);
        }
    };

    for name in &expanded_names {
        if let Err(e) =
            resolve_and_add_manifest(name, &installed_packages, &mut manifests_to_uninstall)
        {
            eprintln!("{}", e);
            failed_resolution = true;
        }
    }

    if failed_resolution {
        std::process::exit(1);
    }

    if manifests_to_uninstall.is_empty() {
        println!("No packages to uninstall.");
        return;
    }

    manifests_to_uninstall.sort_by(|a, b| a.name.cmp(&b.name));
    manifests_to_uninstall.dedup_by(|a, b| {
        a.name == b.name
            && a.sub_package == b.sub_package
            && a.repo == b.repo
            && a.registry_handle == b.registry_handle
    });

    let mut total_size_freed_bytes: u64 = 0;
    for manifest in &manifests_to_uninstall {
        let mut package_size: u64 = 0;
        for file_path_str in &manifest.installed_files {
            let path = Path::new(file_path_str);
            if !path.exists() {
                continue;
            }
            if path.is_dir() {
                package_size += WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.metadata().ok())
                    .filter(|m| m.is_file())
                    .map(|m| m.len())
                    .sum::<u64>();
            } else if let Ok(metadata) = fs::metadata(path) {
                package_size += metadata.len();
            }
        }
        total_size_freed_bytes += package_size;
    }

    println!("Packages to remove:");
    for manifest in &manifests_to_uninstall {
        let source_str = if let Some(sub) = &manifest.sub_package {
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
        println!("  - {}", source_str);
    }

    println!(
        "\nTotal size to be freed: {}",
        crate::utils::format_bytes(total_size_freed_bytes)
    );

    if !crate::utils::ask_for_confirmation(":: Proceed with removal?", yes) {
        let _ = lock::release_lock();
        return;
    }

    let transaction = match transaction::begin() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to begin transaction: {}", e);
            std::process::exit(1);
        }
    };

    let mut failed_packages = Vec::new();
    let mut successfully_uninstalled = Vec::new();

    for manifest in &manifests_to_uninstall {
        let source_str = if let Some(sub) = &manifest.sub_package {
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

        println!(
            "--- Uninstalling package '{}' ---",
            source_str.blue().bold()
        );

        match pkg::uninstall::run(&source_str, scope_override) {
            Ok(uninstalled_manifest) => {
                if let Err(e) = transaction::record_operation(
                    &transaction.id,
                    types::TransactionOperation::Uninstall {
                        manifest: Box::new(uninstalled_manifest),
                    },
                ) {
                    eprintln!(
                        "Failed to record transaction operation for {}: {}",
                        source_str, e
                    );
                    failed_packages.push(source_str.clone());
                } else {
                    successfully_uninstalled.push(source_str.clone());
                    println!("\n{} Uninstallation complete.", "Success:".green());
                }
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                failed_packages.push(source_str.clone());
            }
        }
    }

    if !failed_packages.is_empty() {
        eprintln!("\nError: Uninstallation failed for some packages.");
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
    } else if let Err(e) = transaction::commit(&transaction.id) {
        eprintln!("Warning: Failed to commit transaction: {}", e);
    }

    if save
        && let Err(e) =
            crate::project::config::remove_packages_from_config(&successfully_uninstalled)
    {
        eprintln!(
            "{}: Failed to remove packages from zoi.yaml: {}",
            "Warning".yellow().bold(),
            e
        );
    }
}

fn resolve_and_add_manifest(
    name: &str,
    installed_packages: &[types::InstallManifest],
    manifests_to_uninstall: &mut Vec<types::InstallManifest>,
) -> Result<(), String> {
    let request = match pkg::resolve::parse_source_string(name) {
        Ok(req) => req,
        Err(e) => return Err(format!("Error: Invalid package name '{}': {}", name, e)),
    };

    let mut candidates: Vec<_> = installed_packages
        .iter()
        .filter(|m| {
            let name_matches = m.name == request.name;
            let sub_matches = m.sub_package == request.sub_package;
            name_matches && sub_matches
        })
        .collect();

    if let Some(repo) = &request.repo {
        candidates.retain(|m| m.repo == *repo);
    }
    if let Some(handle) = &request.handle {
        candidates.retain(|m| m.registry_handle == *handle);
    }

    match candidates.len() {
        0 => Err(format!("Error: Package '{}' is not installed.", name)),
        1 => {
            if !manifests_to_uninstall.iter().any(|m| {
                m.name == candidates[0].name
                    && m.sub_package == candidates[0].sub_package
                    && m.repo == candidates[0].repo
                    && m.registry_handle == candidates[0].registry_handle
            }) {
                manifests_to_uninstall.push(candidates[0].clone());
            }
            Ok(())
        }
        _ => {
            let mut error_msg = format!(
                "Error: Ambiguous package name '{}'. It is installed from multiple repositories:\n",
                name
            );
            for manifest in candidates {
                error_msg.push_str(&format!(
                    "  - #{}@{}/{}\n",
                    manifest.registry_handle, manifest.repo, manifest.name
                ));
            }
            error_msg.push_str("Please be more specific, e.g. '#handle@repo/name'.");
            Err(error_msg)
        }
    }
}
