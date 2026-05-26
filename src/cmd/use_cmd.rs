use crate::pkg::{config, types};
use crate::project;
use anyhow::{Result, anyhow};
use colored::*;
use std::collections::HashMap;

pub fn run(packages: Vec<String>, global: bool) -> Result<()> {
    if global {
        run_global(packages)
    } else {
        run_project(packages)
    }
}

fn run_global(packages: Vec<String>) -> Result<()> {
    println!(
        "{} Adding packages to global configuration...",
        "::".bold().blue()
    );

    let mut versions_to_add = HashMap::new();
    for pkg_spec in &packages {
        let request = crate::pkg::resolve::parse_source_string(pkg_spec)?;
        let version = request.version_spec.unwrap_or_else(|| "latest".to_string());
        versions_to_add.insert(request.name, version);
    }

    config::update_global_versions(versions_to_add)?;

    println!("{} Installing global packages...", "::".bold().blue());
    let options = crate::SourceInstallOptions {
        scope_override: Some(types::Scope::User),
        yes: true,
        ..Default::default()
    };

    crate::install_sources(&packages, &options)?;

    println!("\n{}", "Global packages updated and installed.".green());
    Ok(())
}

fn run_project(packages: Vec<String>) -> Result<()> {
    let zoi_yaml = std::path::Path::new("zoi.yaml");
    if !zoi_yaml.exists() {
        return Err(anyhow!(
            "No 'zoi.yaml' found. Run 'zoi use --global' or initialize a project first."
        ));
    }

    println!(
        "{} Adding packages to project configuration...",
        "::".bold().blue()
    );
    project::config::add_packages_to_config(&packages)?;

    println!("{} Installing project packages...", "::".bold().blue());
    let options = crate::SourceInstallOptions {
        scope_override: Some(types::Scope::Project),
        yes: true,
        ..Default::default()
    };

    crate::install_sources(&packages, &options)?;

    println!("\n{}", "Project packages updated and installed.".green());
    Ok(())
}
