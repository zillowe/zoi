use crate::pkg::local;
use colored::*;
use comfy_table::{presets::UTF8_FULL, Table};
use std::collections::HashSet;

pub fn run(args: Vec<String>) {
    let mut list_all = false;
    let mut repo_filter: Option<String> = None;

    for arg in args {
        if arg == "all" {
            list_all = true;
        } else if arg.starts_with('@') {
            repo_filter = Some(arg.strip_prefix('@').unwrap().to_string());
        }
    }

    if list_all {
        if let Err(e) = run_list_all(repo_filter) {
            eprintln!("{}: {}", "Error".red(), e);
        }
    } else {
        if let Err(e) = run_list_installed(repo_filter) {
            eprintln!("{}: {}", "Error".red(), e);
        }
    }
}

fn run_list_installed(repo_filter: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let packages = local::get_installed_packages()?;
    if packages.is_empty() {
        println!("No packages installed by Zoi.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Package", "Version", "Repo"]);

    let mut found_packages = false;
    for pkg in packages {
        if let Some(repo) = &repo_filter {
            if &pkg.repo == repo {
                table.add_row(vec![pkg.name, pkg.version, pkg.repo]);
                found_packages = true;
            }
        } else {
            table.add_row(vec![pkg.name, pkg.version, pkg.repo]);
            found_packages = true;
        }
    }

    if !found_packages {
        if let Some(repo) = repo_filter {
            println!("No packages installed from repo '{}'.", repo);
        }
    } else {
        println!("{table}");
    }

    Ok(())
}

fn run_list_all(repo_filter: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let installed_pkgs = local::get_installed_packages()?
        .into_iter()
        .map(|p| p.name)
        .collect::<HashSet<_>>();

    let available_pkgs = if let Some(repo) = &repo_filter {
        local::get_packages_from_repo(repo)?
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
        .set_header(vec!["Status", "Package", "Version", "Repo"]);

    for pkg in available_pkgs {
        let status = if installed_pkgs.contains(&pkg.name) {
            "✓".green()
        } else {
            "".clear()
        };
        table.add_row(vec![status.to_string(), pkg.name, pkg.version, pkg.repo]);
    }

    println!("{table}");
    Ok(())
}
