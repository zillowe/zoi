use crate::cmd::utils;
use crate::cmd::ux;
use crate::pkg::{self, lock, transaction, types};
use anyhow::{Result, anyhow};
use colored::*;
use mlua::LuaSerdeExt;
use serde_json::json;
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
    explain: bool,
    plan_json: bool,
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
        if let Err(e) = resolve_and_add_manifest(
            name,
            &installed_packages,
            &mut manifests_to_uninstall,
            scope_override,
            yes,
        ) {
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
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "uninstall".to_string(),
            success: 0,
            failed: 0,
            skipped: 0,
        });
        return Ok(());
    }

    manifests_to_uninstall.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| scope_rank(a.scope).cmp(&scope_rank(b.scope)))
            .then_with(|| a.registry_handle.cmp(&b.registry_handle))
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.sub_package.cmp(&b.sub_package))
    });
    manifests_to_uninstall.dedup_by(|a, b| {
        a.name == b.name
            && a.sub_package == b.sub_package
            && a.repo == b.repo
            && a.registry_handle == b.registry_handle
            && a.scope == b.scope
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

    let removal_ids: std::collections::HashSet<String> = manifests_to_uninstall
        .iter()
        .map(removal_identity)
        .collect();
    let mut dangerous = Vec::new();
    let mut impact_json = Vec::new();
    for manifest in &manifests_to_uninstall {
        let package_dir = pkg::local::get_package_dir(
            manifest.scope,
            &manifest.registry_handle,
            &manifest.repo,
            &manifest.name,
        )?;
        let external_dependents =
            collect_external_dependents(&removal_ids, pkg::local::get_dependents(&package_dir)?);

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

        if !external_dependents.is_empty() {
            dangerous.push((source.clone(), external_dependents.clone()));
        }
        impact_json.push(json!({
            "source": source,
            "name": manifest.name,
            "version": manifest.version,
            "sub_package": manifest.sub_package,
            "scope": format!("{:?}", manifest.scope),
            "registry": manifest.registry_handle,
            "repo": manifest.repo,
            "external_dependents": external_dependents,
            "installed_files": manifest.installed_files.len(),
        }));
    }

    let preflight = ux::PreflightSummary::new("Uninstall preflight")
        .row("Scope override", format!("{:?}", scope_override))
        .row("Recursive", recursive.to_string())
        .row("Packages", manifests_to_uninstall.len().to_string())
        .row("Dangerous removals", dangerous.len().to_string())
        .row(
            "Estimated freed size",
            crate::utils::format_bytes(total_size_freed_bytes),
        );
    ux::print_preflight(&preflight);

    if explain {
        let mut report = ux::ExplainReport::new("Uninstall explanation");
        for manifest in &manifests_to_uninstall {
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
            report = report.item(
                format!("{} [{}]", source, manifest.version),
                format!("reason={:?}", manifest.reason),
                Vec::new(),
            );
        }
        if !dangerous.is_empty() {
            for (source, deps) in &dangerous {
                report = report.item(
                    source.clone(),
                    format!("blocks {} dependent(s)", deps.len()),
                    deps.iter()
                        .map(|dep| format!("dependent: {}", dep))
                        .collect::<Vec<_>>(),
                );
            }
        }
        ux::print_explain(&report);
    }

    if plan_json {
        let plan = json!({
            "recursive": recursive,
            "scope_override": format!("{:?}", scope_override),
            "totals": {
                "packages": manifests_to_uninstall.len(),
                "dangerous_removals": dangerous.len(),
                "freed_bytes": total_size_freed_bytes,
            },
            "packages": impact_json,
        });
        ux::emit_plan_json_v1("uninstall", plan)?;
    }

    if !dangerous.is_empty() {
        println!(
            "\n{} Removing these packages will break dependents:",
            "Warning".yellow().bold()
        );
        for (source, deps) in &dangerous {
            println!("  - {}", source.cyan());
            for dep in deps {
                println!("    * {}", dep);
            }
        }
        if !crate::utils::ask_for_confirmation("Dangerous removal detected. Continue anyway?", yes)
        {
            ux::print_transaction_summary(&ux::TransactionSummary {
                command: "uninstall".to_string(),
                success: 0,
                failed: 0,
                skipped: manifests_to_uninstall.len(),
            });
            return Ok(());
        }
    }

    if !crate::utils::ask_for_confirmation(":: Proceed with removal?", yes) {
        let _ = lock::release_lock();
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "uninstall".to_string(),
            success: 0,
            failed: 0,
            skipped: manifests_to_uninstall.len(),
        });
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
            "{} Uninstalling package '{}'...",
            "::".bold().blue(),
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
                    plugin_manager.trigger_hook_nonfatal("on_post_uninstall", Some(pkg_val));
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
        eprintln!("\n{} Rolling back changes...", "::".bold().yellow());
        if let Err(e) = transaction::rollback(&transaction.id) {
            eprintln!("\nCRITICAL: Rollback failed: {}", e);
            eprintln!(
                "The system may be in an inconsistent state. The transaction log is at ~/.zoi/transactions/{}.json",
                transaction.id
            );
        } else {
            println!("\n{} Rollback successful.", "Success:".green().bold());
        }
        ux::print_transaction_summary(&ux::TransactionSummary {
            command: "uninstall".to_string(),
            success: successfully_uninstalled.len(),
            failed: failed_packages.len(),
            skipped: 0,
        });
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
    ux::print_transaction_summary(&ux::TransactionSummary {
        command: "uninstall".to_string(),
        success: successfully_uninstalled.len(),
        failed: 0,
        skipped: 0,
    });
    Ok(())
}

fn removal_identity(manifest: &types::InstallManifest) -> String {
    if let Some(sub) = &manifest.sub_package {
        format!("{}@{}:{}", manifest.name, manifest.version, sub)
    } else {
        format!("{}@{}", manifest.name, manifest.version)
    }
}

fn scope_rank(scope: types::Scope) -> u8 {
    match scope {
        types::Scope::Project => 0,
        types::Scope::User => 1,
        types::Scope::System => 2,
    }
}

fn collect_external_dependents(
    removal_ids: &std::collections::HashSet<String>,
    dependents: Vec<String>,
) -> Vec<String> {
    let mut external = dependents
        .into_iter()
        .filter(|dep| !removal_ids.contains(dep))
        .collect::<Vec<_>>();
    external.sort();
    external
}

fn resolve_and_add_manifest(
    name: &str,
    installed_packages: &[types::InstallManifest],
    manifests_to_uninstall: &mut Vec<types::InstallManifest>,
    scope_override: Option<types::Scope>,
    yes: bool,
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
            let scope_matches = scope_override.is_none_or(|scope| m.scope == scope);
            name_matches && sub_matches && scope_matches
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
            let owned_candidates = candidates.into_iter().cloned().collect::<Vec<_>>();
            let chosen = crate::cmd::installed_select::choose_installed_manifest(
                name,
                &owned_candidates,
                yes,
            )
            .map_err(|e| format!("Error: {}", e))?;

            if !manifests_to_uninstall.iter().any(|m| {
                m.name == chosen.name
                    && m.sub_package == chosen.sub_package
                    && m.repo == chosen.repo
                    && m.registry_handle == chosen.registry_handle
                    && m.scope == chosen.scope
            }) {
                manifests_to_uninstall.push(chosen);
            }
            Ok(())
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

                    let matching_dep_manifests = installed_packages
                        .iter()
                        .filter(|m| {
                            m.name == dep_req.name
                                && m.sub_package == dep_req.sub_package
                                && dep_req.repo.as_ref().is_none_or(|repo| m.repo == *repo)
                                && dep_req
                                    .handle
                                    .as_ref()
                                    .is_none_or(|handle| m.registry_handle == *handle)
                                && dep_req
                                    .version_spec
                                    .as_ref()
                                    .is_none_or(|version| m.version == *version)
                        })
                        .collect::<Vec<_>>();

                    if matching_dep_manifests.len() == 1 {
                        let dm = matching_dep_manifests[0];
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

#[cfg(test)]
mod tests {
    use super::collect_external_dependents;
    use std::collections::HashSet;

    #[test]
    fn dangerous_removal_ignores_dependents_in_same_removal_set() {
        let mut removal_ids = HashSet::new();
        removal_ids.insert("foo@1.0.0".to_string());

        let external = collect_external_dependents(
            &removal_ids,
            vec![
                "foo@1.0.0".to_string(),
                "bar@2.0.0".to_string(),
                "baz@3.0.0".to_string(),
            ],
        );

        assert_eq!(
            external,
            vec!["bar@2.0.0".to_string(), "baz@3.0.0".to_string()]
        );
    }

    #[test]
    fn dangerous_removal_dependents_are_sorted_for_stable_output() {
        let removal_ids = HashSet::new();
        let external = collect_external_dependents(
            &removal_ids,
            vec!["c@1".to_string(), "a@1".to_string(), "b@1".to_string()],
        );
        assert_eq!(
            external,
            vec!["a@1".to_string(), "b@1".to_string(), "c@1".to_string()]
        );
    }
}
