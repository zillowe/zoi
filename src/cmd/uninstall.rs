use crate::cmd::utils;
use crate::pkg::{self, lock, transaction, types};
use anyhow::{Result, anyhow};
use colored::*;
use mlua::LuaSerdeExt;
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
    recursive: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
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

    if save && scope_override != Some(types::Scope::Project) {
        return Err(anyhow!(
            "The --save flag can only be used with project-scoped uninstalls."
        ));
    }

    let installed_packages = pkg::local::get_installed_packages()?;

    let mut manifests_to_uninstall: Vec<types::InstallManifest> = Vec::new();
    let mut failed_resolution = false;

    let expanded_names = utils::expand_split_packages(package_names, "Uninstalling")?;

    for name in &expanded_names {
        if let Err(e) =
            resolve_and_add_manifest(name, &installed_packages, &mut manifests_to_uninstall)
        {
            eprintln!("{}", e);
            failed_resolution = true;
        }
    }

    if failed_resolution {
        return Err(anyhow!(
            "Failed to resolve some packages for uninstallation."
        ));
    }

    if recursive {
        collect_recursive_uninstalls(&mut manifests_to_uninstall, &installed_packages)?;
    }

    if manifests_to_uninstall.is_empty() {
        println!("No packages to uninstall.");
        return Ok(());
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
        return Ok(());
    }

    let transaction = transaction::begin()?;

    let mut failed_packages = Vec::new();
    let mut successfully_uninstalled = Vec::new();

    for manifest in &manifests_to_uninstall {
        let pkg_val = plugin_manager
            .lua
            .to_value(manifest)
            .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
        plugin_manager.trigger_hook("on_pre_uninstall", Some(pkg_val.clone()))?;

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

        match pkg::uninstall::run(&source_str, scope_override, yes) {
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
                    plugin_manager.trigger_hook("on_post_uninstall", Some(pkg_val))?;
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
        return Err(anyhow!(
            "Uninstallation failed for: {}",
            failed_packages.join(", ")
        ));
    } else {
        if let Ok(modified_files) = transaction::get_modified_files(&transaction.id) {
            let _ = crate::pkg::hooks::global::run_global_hooks(
                crate::pkg::hooks::global::HookWhen::PostTransaction,
                &modified_files,
                "remove",
            );
        }

        if let Err(e) = transaction::commit(&transaction.id) {
            eprintln!("Warning: Failed to commit transaction: {}", e);
        }
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
    Ok(())
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

fn collect_recursive_uninstalls(
    manifests_to_uninstall: &mut Vec<types::InstallManifest>,
    installed_packages: &[types::InstallManifest],
) -> Result<()> {
    let mut changed = true;
    while changed {
        changed = false;
        let mut new_to_add = Vec::new();

        for manifest in manifests_to_uninstall.iter() {
            for dep_str in &manifest.installed_dependencies {
                if let Ok(dep) = pkg::dependencies::parse_dependency_string(dep_str)
                    && dep.manager == "zoi"
                {
                    let dep_req = match pkg::resolve::parse_source_string(dep.package) {
                        Ok(req) => req,
                        Err(_) => continue,
                    };

                    let dep_manifest = installed_packages
                        .iter()
                        .find(|m| m.name == dep_req.name && m.sub_package == dep_req.sub_package);

                    if let Some(dm) = dep_manifest {
                        if !matches!(dm.reason, types::InstallReason::Dependency { .. }) {
                            continue;
                        }

                        if manifests_to_uninstall
                            .iter()
                            .any(|m| m.name == dm.name && m.sub_package == dm.sub_package)
                            || new_to_add.iter().any(|m: &&types::InstallManifest| {
                                m.name == dm.name && m.sub_package == dm.sub_package
                            })
                        {
                            continue;
                        }

                        let pkg_dir = pkg::local::get_package_dir(
                            dm.scope,
                            &dm.registry_handle,
                            &dm.repo,
                            &dm.name,
                        )?;
                        let dependents = pkg::local::get_dependents(&pkg_dir)?;

                        let all_dependents_will_be_removed = dependents.iter().all(|dep_id| {
                            manifests_to_uninstall.iter().any(|m| {
                                let m_id = if let Some(sub) = &m.sub_package {
                                    format!("{}@{}:{}", m.name, m.version, sub)
                                } else {
                                    format!("{}@{}", m.name, m.version)
                                };
                                m_id == *dep_id
                            })
                        });

                        if all_dependents_will_be_removed {
                            new_to_add.push(dm);
                            changed = true;
                        }
                    }
                }
            }
        }

        for nm in new_to_add {
            manifests_to_uninstall.push(nm.clone());
        }
    }
    Ok(())
}
