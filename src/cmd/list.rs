use crate::pkg::{local, types};

use comfy_table::{Table, presets::UTF8_FULL};
use std::collections::HashSet;
use std::io::{self, Write};
use std::process::{Command, Stdio};

pub fn run(
    all: bool,
    repo_filter: Option<String>,
    type_filter: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let package_type = match type_filter.as_deref() {
        Some("package") => Some(types::PackageType::Package),
        Some("collection") => Some(types::PackageType::Collection),
        Some("service") => Some(types::PackageType::Service),
        Some("config") => Some(types::PackageType::Config),
        Some("app") => Some(types::PackageType::App),
        Some("extension") => Some(types::PackageType::Extension),
        Some("library") => Some(types::PackageType::Library),
        Some(other) => return Err(format!("Invalid package type: {}", other).into()),
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
) -> Result<(), Box<dyn std::error::Error>> {
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
        if let Some(repo) = &repo_filter
            && !pkg.repo.starts_with(repo)
        {
            continue;
        }
        if type_filter.is_some() && pkg.package_type != type_filter.unwrap() {
            continue;
        }

        table.add_row(vec![
            pkg.name,
            pkg.version,
            pkg.repo,
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
) -> Result<(), Box<dyn std::error::Error>> {
    let installed_pkgs = local::get_installed_packages()?
        .into_iter()
        .map(|p| p.name)
        .collect::<HashSet<_>>();

    let mut available_pkgs = local::get_all_available_packages()?;

    if let Some(repo) = &repo_filter {
        available_pkgs.retain(|pkg| pkg.repo.starts_with(repo));
    }

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

        let status = if installed_pkgs.contains(&pkg.name) {
            "✓"
        } else {
            ""
        };
        let version =
            crate::pkg::resolve::get_default_version(&pkg).unwrap_or_else(|_| "N/A".to_string());
        table.add_row(vec![
            status.to_string(),
            pkg.name,
            version,
            pkg.repo,
            format!("{:?}", pkg.package_type),
        ]);
    }

    print_with_pager(&table.to_string())?;
    Ok(())
}
