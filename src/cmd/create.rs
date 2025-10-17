use crate::pkg::{dependencies, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

#[derive(Parser)]
pub struct CreateCommand {
    /// The source of the package (name, @repo/name, path to .pkg.lua, or URL)
    pub source: String,
    /// The application name to substitute into template commands
    pub app_name: String,
}

pub fn run(args: CreateCommand, yes: bool) {
    if let Err(e) = run_pkg_create(&args.source, &args.app_name, yes) {
        eprintln!("{}: {}", "Error".red().bold(), e);
    }
}

fn run_pkg_create(source: &str, app_name: &str, yes: bool) -> Result<()> {
    let app_dir = Path::new(app_name);
    if app_dir.exists() {
        if app_dir.is_dir() {
            if fs::read_dir(app_dir)?.next().is_some() {
                println!(
                    "{}",
                    format!(
                        "Warning: Directory '{}' already exists and is not empty.",
                        app_name
                    )
                    .yellow()
                );
                if !utils::ask_for_confirmation("Do you want to continue?", yes) {
                    return Err(anyhow!("Operation aborted by user."));
                }
            }
        } else {
            return Err(anyhow!(
                "A file with the name '{}' already exists.",
                app_name
            ));
        }
    }

    let (pkg, version, _, _, registry_handle) = resolve::resolve_package_and_version(source)?;

    if pkg.package_type != types::PackageType::App {
        return Err(anyhow!(
            "Package '{}' is not of type 'app' (found {:?}).",
            pkg.name,
            pkg.package_type
        ));
    }

    let platform = utils::get_platform()?;
    let methods = pkg
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Package '{}' has no app commands.", pkg.name))?;

    let method = methods
        .iter()
        .find(|m| utils::is_platform_compatible(&platform, &m.platforms))
        .ok_or_else(|| {
            anyhow!(
                "No compatible appCreate commands for platform '{}'.",
                platform
            )
        })?;

    if let Some(deps) = &pkg.dependencies {
        let handle = registry_handle.as_deref().unwrap_or("local");
        let parent_id = format!("#{}@{}", handle, pkg.repo);
        let processed_deps = Mutex::new(HashSet::new());
        let mut _installed_deps_list: Vec<String> = Vec::new();

        if let Some(build_deps) = &deps.build {
            dependencies::resolve_and_install_required(
                &build_deps.get_required_simple(),
                &parent_id,
                &version,
                pkg.scope,
                yes,
                false,
                &processed_deps,
                &mut _installed_deps_list,
            )?;
            let mut chosen_options = Vec::new();
            dependencies::resolve_and_install_required_options(
                &build_deps.get_required_options(),
                &parent_id,
                &version,
                pkg.scope,
                yes,
                false,
                &processed_deps,
                &mut _installed_deps_list,
                &mut chosen_options,
            )?;
        }

        if let Some(runtime_deps) = &deps.runtime {
            dependencies::resolve_and_install_required(
                &runtime_deps.get_required_simple(),
                &parent_id,
                &version,
                pkg.scope,
                yes,
                false,
                &processed_deps,
                &mut _installed_deps_list,
            )?;
            let mut chosen_options = Vec::new();
            dependencies::resolve_and_install_required_options(
                &runtime_deps.get_required_options(),
                &parent_id,
                &version,
                pkg.scope,
                yes,
                false,
                &processed_deps,
                &mut _installed_deps_list,
                &mut chosen_options,
            )?;
        }
    }

    println!(
        "Creating app '{}' using package '{}'...",
        app_name.cyan(),
        pkg.name.green()
    );

    let create_cmd = method
        .app_create
        .replace("${appName}", app_name)
        .replace("{appName}", app_name);

    println!("Executing: {}", create_cmd.cyan());
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("pwsh")
            .arg("-Command")
            .arg(&create_cmd)
            .output()?
    } else {
        std::process::Command::new("bash")
            .arg("-c")
            .arg(&create_cmd)
            .output()?
    };

    if !output.status.success() {
        use std::io::Write;
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
        return Err(anyhow!("Create command failed: '{}'", create_cmd));
    }

    if let Some(extra_cmds) = &method.commands {
        for cmd in extra_cmds {
            let final_cmd = cmd
                .replace("${appName}", app_name)
                .replace("{appName}", app_name);

            println!("Executing: {}", final_cmd.cyan());
            let output = if cfg!(target_os = "windows") {
                std::process::Command::new("pwsh")
                    .arg("-Command")
                    .arg(&final_cmd)
                    .output()?
            } else {
                std::process::Command::new("bash")
                    .arg("-c")
                    .arg(&final_cmd)
                    .output()?
            };

            if !output.status.success() {
                use std::io::Write;
                std::io::stdout().write_all(&output.stdout)?;
                std::io::stderr().write_all(&output.stderr)?;
                return Err(anyhow!("Command failed: '{}'", final_cmd));
            }
        }
    }

    println!("{}", "App created successfully.".green());
    Ok(())
}
