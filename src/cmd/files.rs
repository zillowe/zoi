use crate::pkg::local;
use anyhow::{Result, anyhow};
use colored::*;

pub fn run(package_name: &str) {
    if let Err(e) = run_impl(package_name) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run_impl(package_name: &str) -> Result<()> {
    let installed_packages = local::get_installed_packages()?;

    let Some(pkg) = installed_packages.iter().find(|p| p.name == package_name) else {
        return Err(anyhow!("Package '{}' is not installed.", package_name));
    };

    println!("Files for {} {}:", pkg.name.cyan(), pkg.version.yellow());

    if pkg.installed_files.is_empty() {
        println!("(No files recorded for this package)");
    } else {
        let mut sorted_files = pkg.installed_files.clone();
        sorted_files.sort();
        for file in &sorted_files {
            println!("{}", file);
        }
    }

    Ok(())
}
