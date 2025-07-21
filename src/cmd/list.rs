use crate::pkg::local;
use colored::*;
use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;

pub fn run(list_all: bool) {
    if list_all {
        if let Err(e) = run_list_all() {
            eprintln!("{}: {}", "Error".red(), e);
        }
    } else if let Err(e) = run_list_installed() {
        eprintln!("{}: {}", "Error".red(), e);
    }
}

fn run_list_installed() -> Result<(), Box<dyn std::error::Error>> {
    let packages = local::get_installed_packages()?;
    if packages.is_empty() {
        println!("No packages installed by Zoi.");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec!["Package", "Version", "Repo"]);
    for pkg in packages {
        table.add_row(vec![pkg.name, pkg.version, pkg.repo]);
    }
    println!("{table}");
    Ok(())
}

fn run_list_all() -> Result<(), Box<dyn std::error::Error>> {
    let installed_pkgs =
        local::get_installed_packages()?.into_iter().map(|p| p.name).collect::<HashSet<_>>();
    let available_pkgs = local::get_all_available_packages()?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec!["Status", "Package", "Version", "Repo"]);

    for pkg in available_pkgs {
        let status = if installed_pkgs.contains(&pkg.name) { "✓".green() } else { "".clear() };
        table.add_row(vec![status.to_string(), pkg.name, pkg.version, pkg.repo]);
    }
    println!("{table}");
    Ok(())
}
