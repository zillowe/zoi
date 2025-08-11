use crate::pkg::{config_handler, dependencies, local, resolve, types};
use crate::utils;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

fn get_bin_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("bin"))
}

fn run_post_uninstall_hooks(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if let Some(hooks) = &pkg.post_uninstall {
        println!("\n{}", "Running post-uninstallation commands...".bold());
        let platform = utils::get_platform()?;
        let version = pkg.version.as_deref().unwrap_or("");

        for hook in hooks {
            if utils::is_platform_compatible(&platform, &hook.platforms) {
                for cmd_str in &hook.commands {
                    let final_cmd = cmd_str
                        .replace("{version}", version)
                        .replace("{name}", &pkg.name);

                    println!("Executing: {}", final_cmd.cyan());

                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                            .template("{spinner:.green} {msg}")?,
                    );
                    pb.set_message(format!("Running: {}", final_cmd));

                    let output = if cfg!(target_os = "windows") {
                        Command::new("pwsh")
                            .arg("-Command")
                            .arg(&final_cmd)
                            .output()?
                    } else {
                        Command::new("bash").arg("-c").arg(&final_cmd).output()?
                    };

                    pb.finish_and_clear();

                    if !output.status.success() {
                        io::stdout().write_all(&output.stdout)?;
                        io::stderr().write_all(&output.stderr)?;
                        return Err(
                            format!("Post-uninstall command failed: '{}'", final_cmd).into()
                        );
                    } else {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if !stdout.trim().is_empty() {
                            println!("{}", stdout.trim());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn uninstall_collection(
    pkg: &types::Package,
    manifest: &types::InstallManifest,
    scope: types::Scope,
) -> Result<(), Box<dyn Error>> {
    println!("Uninstalling collection '{}'...", pkg.name.bold());

    let dependencies_to_uninstall = &manifest.installed_dependencies;

    if dependencies_to_uninstall.is_empty() {
        println!("Collection has no dependencies to uninstall.");
    } else {
        println!("Uninstalling dependencies of the collection...");
        for dep_str in dependencies_to_uninstall {
            println!("\n--- Uninstalling dependency: {} ---", dep_str.bold());
            if let Err(e) = dependencies::uninstall_dependency(dep_str, &run) {
                eprintln!(
                    "Warning: Could not uninstall dependency '{}': {}",
                    dep_str, e
                );
            }
        }
    }

    local::remove_manifest(&pkg.name, scope)?;
    println!("\nRemoved manifest for collection '{}'.", pkg.name);

    match crate::pkg::telemetry::posthog_capture_event("uninstall", pkg, env!("CARGO_PKG_VERSION"))
    {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    Ok(())
}

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    let (pkg, _) = resolve::resolve_package_and_version(package_name)?;
    let (manifest, scope) =
        if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::User)? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::System)? {
            (m, types::Scope::System)
        } else {
            return Err(format!("Package '{}' is not installed by Zoi.", package_name).into());
        };

    if pkg.package_type == types::PackageType::Collection {
        return uninstall_collection(&pkg, &manifest, scope);
    }

    let pkg_id = format!("zoi:{}", pkg.name);
    let dependents = dependencies::get_dependents(&pkg_id)?;
    if !dependents.is_empty() {
        return Err(format!(
            "Cannot uninstall '{}' because other packages depend on it:\n  -{}\n\nPlease uninstall these packages first.",
            &pkg.name,
            dependents.join("\n  - ")
        ).into());
    }
    let dependencies_to_check = &manifest.installed_dependencies;
    println!(
        "Uninstalling '{}' and its unused dependencies...",
        pkg.name.bold()
    );
    if pkg.package_type == types::PackageType::Config {
        config_handler::run_uninstall_commands(&pkg)?;
    }
    let store_dir = local::get_store_root(scope)?.join(&pkg.name);
    if store_dir.exists() {
        println!("Removing stored files from {}...", store_dir.display());
        fs::remove_dir_all(&store_dir)?;
        println!("{}", "Successfully removed stored files.".green());
    }
    let symlink_path = get_bin_root()?.join(&pkg.name);
    if symlink_path.exists() {
        println!("Removing symlink from {}...", symlink_path.display());
        fs::remove_file(&symlink_path)?;
        println!("{}", "Successfully removed symlink.".green());
    }
    if let Err(e) = run_post_uninstall_hooks(&pkg) {
        eprintln!(
            "{} Post-uninstallation commands failed: {}",
            "Warning:".yellow(),
            e
        );
    }
    for dep_str in dependencies_to_check {
        dependencies::remove_dependency_link(&pkg.name, dep_str)?;
        let other_dependents = dependencies::get_dependents(dep_str)?;
        if other_dependents.is_empty() {
            println!(
                "\n--- Dependency '{}' is no longer needed, uninstalling... ---",
                dep_str.bold()
            );
            if let Err(e) = dependencies::uninstall_dependency(dep_str, &run) {
                eprintln!(
                    "Warning: Could not uninstall dependency '{}': {}",
                    dep_str, e
                );
            }
        } else {
            println!(
                "Info: Dependency '{}' is still needed by: {}",
                dep_str.yellow(),
                other_dependents.join(", ")
            );
        }
    }

    local::remove_manifest(&pkg.name, scope)?;
    println!("Removed manifest for '{}'.", pkg.name);

    match crate::pkg::telemetry::posthog_capture_event("uninstall", &pkg, env!("CARGO_PKG_VERSION"))
    {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    Ok(())
}
