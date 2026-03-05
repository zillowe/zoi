use crate::pkg::{config, local, types};
use anyhow::{Result, anyhow};
use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub fn run(
    all: bool,
    registry_filter: Option<String>,
    repo_filter: Option<String>,
    type_filter: Option<String>,
    foreign: bool,
    names_only: bool,
    completion: bool,
) -> Result<()> {
    let package_type = match type_filter.as_deref() {
        Some("package") => Some(types::PackageType::Package),
        Some("collection") => Some(types::PackageType::Collection),
        Some("app") => Some(types::PackageType::App),
        Some("extension") => Some(types::PackageType::Extension),
        Some(other) => return Err(anyhow!("Invalid package type: {}", other)),
        None => None,
    };

    if names_only || completion {
        return run_list_names(all, registry_filter, repo_filter, package_type, completion);
    }

    if all {
        if foreign {
            return Err(anyhow!("The --foreign flag cannot be used with --all."));
        }
        run_list_all(registry_filter, repo_filter, package_type)?;
    } else {
        run_list_installed(registry_filter, repo_filter, package_type, foreign)?;
    }
    Ok(())
}

fn print_with_pager(content: &str) -> io::Result<()> {
    let pager = if crate::utils::command_exists("less") {
        "less"
    } else if crate::utils::command_exists("more") {
        "more"
    } else {
        print!("{}", content);
        return Ok(());
    };

    let mut command = Command::new(pager);
    if pager == "less" {
        command.arg("-R");
    }

    let mut child = command.stdin(Stdio::piped()).spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(content.as_bytes());
    }

    child.wait()?;
    Ok(())
}

fn run_list_installed(
    registry_filter: Option<String>,
    repo_filter: Option<String>,
    type_filter: Option<types::PackageType>,
    foreign: bool,
) -> Result<()> {
    let config = config::read_config()?;
    let mut active_registries = HashSet::new();
    if let Some(default) = &config.default_registry {
        active_registries.insert(default.handle.clone());
    }
    for reg in &config.added_registries {
        active_registries.insert(reg.handle.clone());
    }

    let mut db_failed = false;
    let packages_from_db = match crate::pkg::db::list_all_packages("local") {
        Ok(pkgs) => pkgs,
        Err(_) => {
            db_failed = true;
            Vec::new()
        }
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Package", "Version", "Repo", "Registry", "Type"]);

    let mut found_packages = false;

    if !db_failed && !packages_from_db.is_empty() {
        for pkg in packages_from_db {
            if foreign
                && let Some(reg) = &pkg.registry_handle
                && active_registries.contains(reg)
            {
                continue;
            }

            if let Some(registry_filter) = &registry_filter
                && pkg.registry_handle.as_deref() != Some(registry_filter)
            {
                continue;
            }

            if let Some(repo_filter) = &repo_filter {
                let repo_matches = if repo_filter.contains('/') {
                    pkg.repo == *repo_filter
                } else {
                    pkg.repo.split('/').any(|part| part == *repo_filter)
                };
                if !repo_matches {
                    continue;
                }
            }
            if type_filter.is_some() && pkg.package_type != type_filter.unwrap() {
                continue;
            }

            let package_display = if let Some(sub) = &pkg.sub_package {
                format!("{}:{}", pkg.name, sub)
            } else {
                pkg.name
            };

            let repo_display = pkg.repo.split_once('/').map(|x| x.1).unwrap_or(&pkg.repo);

            table.add_row(vec![
                package_display,
                pkg.version.unwrap_or_else(|| "N/A".to_string()),
                repo_display.to_string(),
                pkg.registry_handle.unwrap_or_else(|| "none".to_string()),
                format!("{:?}", pkg.package_type),
            ]);
            found_packages = true;
        }
    } else {
        let packages = local::get_installed_packages_with_type()?;
        if packages.is_empty() {
            println!("No packages installed by Zoi.");
            return Ok(());
        }

        for pkg in packages {
            let manifest = local::is_package_installed(
                &pkg.name,
                pkg.sub_package.as_deref(),
                types::Scope::User,
            )?
            .or(local::is_package_installed(
                &pkg.name,
                pkg.sub_package.as_deref(),
                types::Scope::System,
            )?)
            .or(local::is_package_installed(
                &pkg.name,
                pkg.sub_package.as_deref(),
                types::Scope::Project,
            )?);

            let Some(m) = manifest else { continue };

            if foreign && active_registries.contains(&m.registry_handle) {
                continue;
            }

            if let Some(registry_filter) = &registry_filter
                && m.registry_handle != *registry_filter
            {
                continue;
            }
            if let Some(repo_filter) = &repo_filter {
                let repo_matches = if repo_filter.contains('/') {
                    pkg.repo == *repo_filter
                } else {
                    pkg.repo.split('/').any(|part| part == *repo_filter)
                };
                if !repo_matches {
                    continue;
                }
            }
            if type_filter.is_some() && pkg.package_type != type_filter.unwrap() {
                continue;
            }

            let package_display = if let Some(sub) = pkg.sub_package {
                format!("{}:{}", pkg.name, sub)
            } else {
                pkg.name
            };

            let repo_display = pkg.repo.split_once('/').map(|x| x.1).unwrap_or(&pkg.repo);

            table.add_row(vec![
                package_display,
                pkg.version,
                repo_display.to_string(),
                m.registry_handle,
                format!("{:?}", pkg.package_type),
            ]);
            found_packages = true;
        }
    }

    if !found_packages {
        println!("No installed packages match your criteria.");
    } else {
        print_with_pager(&table.to_string())?;
    }

    Ok(())
}

fn run_list_names(
    all: bool,
    registry_filter: Option<String>,
    repo_filter: Option<String>,
    type_filter: Option<types::PackageType>,
    completion: bool,
) -> Result<()> {
    let mut entries = HashSet::new();
    let config = config::read_config()?;

    if all {
        let mut registries = Vec::new();
        if let Some(reg) = registry_filter {
            registries.push(reg);
        } else {
            if let Some(default) = &config.default_registry {
                registries.push(default.handle.clone());
            }
            for reg in &config.added_registries {
                registries.push(reg.handle.clone());
            }
        }

        let default_handle = config.default_registry.as_ref().map(|r| &r.handle);

        for handle in registries {
            if let Ok(pkgs) = crate::pkg::db::get_packages_for_completion(&handle) {
                let is_default = default_handle == Some(&handle);
                for pkg in pkgs {
                    if let Some(repo_f) = &repo_filter
                        && !pkg.repo.contains(repo_f)
                    {
                        continue;
                    }

                    let base_name = if is_default {
                        format!("@{}/{}", pkg.repo, pkg.name)
                    } else {
                        format!("#{}@{}/{}", handle, pkg.repo, pkg.name)
                    };

                    let name_with_sub = if let Some(sub) = &pkg.sub_package {
                        format!("{}:{}", base_name, sub)
                    } else {
                        base_name
                    };

                    let entry = if completion {
                        format!("{}:{}", name_with_sub, pkg.description.replace(':', " "))
                    } else {
                        name_with_sub
                    };
                    entries.insert(entry);
                }
            }
        }
    } else {
        let installed = local::get_installed_packages_with_type()?;
        for pkg in installed {
            if let Some(type_f) = type_filter
                && pkg.package_type != type_f
            {
                continue;
            }
            let name = if let Some(sub) = pkg.sub_package {
                format!("{}:{}", pkg.name, sub)
            } else {
                pkg.name
            };

            let entry = name;
            entries.insert(entry);
        }
    }

    let mut sorted_entries: Vec<_> = entries.into_iter().collect();
    sorted_entries.sort();
    for entry in sorted_entries {
        println!("{}", entry);
    }

    Ok(())
}

fn run_list_all(
    registry_filter: Option<String>,
    repo_filter: Option<String>,
    type_filter: Option<types::PackageType>,
) -> Result<()> {
    let installed_pkgs = local::get_installed_packages()?
        .into_iter()
        .map(|p| {
            if let Some(sub) = p.sub_package {
                format!("{}:{}", p.name, sub)
            } else {
                p.name
            }
        })
        .collect::<HashSet<_>>();

    let config = config::read_config()?;

    let mut all_available = Vec::new();
    let mut db_failed = false;

    if let Some(reg_handle) = &registry_filter {
        match crate::pkg::db::list_all_packages(reg_handle) {
            Ok(pkgs) => all_available.extend(pkgs),
            Err(_) => db_failed = true,
        }
    } else {
        let mut registries = Vec::new();
        if let Some(default) = &config.default_registry {
            registries.push(default.handle.clone());
        }
        for reg in &config.added_registries {
            registries.push(reg.handle.clone());
        }

        for handle in registries {
            if handle.is_empty() {
                continue;
            }
            match crate::pkg::db::list_all_packages(&handle) {
                Ok(pkgs) => all_available.extend(pkgs),
                Err(_) => {
                    db_failed = true;
                    break;
                }
            }
        }
    }

    let available_pkgs = if db_failed
        || (all_available.is_empty() && repo_filter.is_none() && registry_filter.is_none())
    {
        if let Some(reg_handle) = &registry_filter {
            let all_repo_names = config::get_all_repos()?;
            let full_repos: Vec<String> = all_repo_names
                .into_iter()
                .map(|r_name| format!("{}/{}", reg_handle, r_name))
                .filter(|full_repo_name| {
                    if let Some(repo_f) = &repo_filter {
                        if repo_f.contains('/') {
                            full_repo_name == repo_f
                        } else {
                            full_repo_name.split('/').any(|part| part == repo_f)
                        }
                    } else {
                        true
                    }
                })
                .collect();
            local::get_packages_from_repos(&full_repos)?
        } else if let Some(repo_filter) = &repo_filter {
            let handle = if let Some(reg) = &config.default_registry {
                reg.handle.clone()
            } else {
                return Err(anyhow!("Default registry not configured."));
            };
            if handle.is_empty() {
                return Err(anyhow!(
                    "Default registry handle is not set. Please run 'zoi sync'.."
                ));
            }
            let all_repo_names = config::get_all_repos()?;
            let repos_to_search: Vec<String> = all_repo_names
                .into_iter()
                .map(|r_name| format!("{}/{}", handle, r_name))
                .filter(|full_repo_name| {
                    if repo_filter.contains('/') {
                        full_repo_name == repo_filter
                    } else {
                        full_repo_name.split('/').any(|part| part == repo_filter)
                    }
                })
                .collect();
            local::get_packages_from_repos(&repos_to_search)?
        } else {
            local::get_all_available_packages()?
        }
    } else {
        if let Some(rf) = &repo_filter {
            all_available.retain(|p| {
                if rf.contains('/') {
                    p.repo == *rf
                } else {
                    p.repo.split('/').any(|part| part == rf)
                }
            });
        }
        all_available
    };

    let handle_for_version = registry_filter.as_deref().or(config
        .default_registry
        .as_ref()
        .map(|reg| reg.handle.as_str()));

    if available_pkgs.is_empty() {
        if let Some(repo) = repo_filter {
            println!("No packages found in repo '{}'.", repo);
        } else {
            println!("No packages found in active repositories.");
        }
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Status", "Package", "Version", "Repo", "Type"]);

    for pkg in available_pkgs {
        if type_filter.is_some() && pkg.package_type != type_filter.unwrap() {
            continue;
        }

        let version = crate::pkg::resolve::get_default_version(&pkg, handle_for_version)
            .unwrap_or_else(|_| "N/A".to_string());
        let repo_display = pkg.repo.split_once('/').map(|x| x.1).unwrap_or(&pkg.repo);

        let full_name = if let Some(sub) = &pkg.sub_package {
            format!("{}:{}", pkg.name, sub)
        } else {
            pkg.name.clone()
        };

        if let Some(subs) = &pkg.sub_packages {
            for sub in subs {
                let full_name_sub = format!("{}:{}", pkg.name, sub);
                let status = if installed_pkgs.contains(&full_name_sub) {
                    "✓"
                } else {
                    ""
                };
                table.add_row(vec![
                    status.to_string(),
                    full_name_sub,
                    version.clone(),
                    repo_display.to_string(),
                    format!("{:?}", pkg.package_type),
                ]);
            }
        } else {
            let status = if installed_pkgs.contains(&full_name) {
                "✓"
            } else {
                ""
            };
            table.add_row(vec![
                status.to_string(),
                full_name,
                version,
                repo_display.to_string(),
                format!("{:?}", pkg.package_type),
            ]);
        }
    }

    print_with_pager(&table.to_string())?;
    Ok(())
}
