use crate::pkg::{config, local, types};
use anyhow::{Result, anyhow};
use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub fn run(all: bool, repo_filter: Option<String>, type_filter: Option<String>) -> Result<()> {
    let package_type = match type_filter.as_deref() {
        Some("package") => Some(types::PackageType::Package),
        Some("collection") => Some(types::PackageType::Collection),
        Some("app") => Some(types::PackageType::App),
        Some("extension") => Some(types::PackageType::Extension),
        Some(other) => return Err(anyhow!("Invalid package type: {}", other)),
        None => None,
    };

    if all {
        run_list_all(repo_filter, package_type)?;
    } else {
        run_list_installed(repo_filter, package_type)?;
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
    repo_filter: Option<String>,
    type_filter: Option<types::PackageType>,
) -> Result<()> {
    let packages = local::get_installed_packages_with_type()?;
    if packages.is_empty() {
        println!("No packages installed by Zoi.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Package", "Version", "Repo", "Type"]);

    let mut found_packages = false;
    for pkg in packages {
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
            format!("{:?}", pkg.package_type),
        ]);
        found_packages = true;
    }

    if !found_packages {
        println!("No installed packages match your criteria.");
    } else {
        print_with_pager(&table.to_string())?;
    }

    Ok(())
}

fn run_list_all(
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
    let handle = config
        .default_registry
        .as_ref()
        .map(|reg| reg.handle.as_str());

    let available_pkgs = if let Some(repo_filter) = &repo_filter {
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
    };

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

        let version = crate::pkg::resolve::get_default_version(&pkg, handle)
            .unwrap_or_else(|_| "N/A".to_string());
        let repo_display = pkg.repo.split_once('/').map(|x| x.1).unwrap_or(&pkg.repo);

        if let Some(subs) = &pkg.sub_packages {
            for sub in subs {
                let full_name = format!("{}:{}", pkg.name, sub);
                let status = if installed_pkgs.contains(&full_name) {
                    "✓"
                } else {
                    ""
                };
                table.add_row(vec![
                    status.to_string(),
                    full_name,
                    version.clone(),
                    repo_display.to_string(),
                    format!("{:?}", pkg.package_type),
                ]);
            }
        } else {
            let status = if installed_pkgs.contains(&pkg.name) {
                "✓"
            } else {
                ""
            };
            table.add_row(vec![
                status.to_string(),
                pkg.name,
                version,
                repo_display.to_string(),
                format!("{:?}", pkg.package_type),
            ]);
        }
    }

    print_with_pager(&table.to_string())?;
    Ok(())
}
