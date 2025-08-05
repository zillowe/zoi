use crate::pkg::{config_handler, local, resolve, types};
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

fn remove_dependent_record(
    dependency_name: &str,
    dependent_pkg_name: &str,
    scope: types::Scope,
) -> Result<(), Box<dyn Error>> {
    let dependent_file = local::get_store_root(scope)?
        .join(dependency_name)
        .join("dependents")
        .join(dependent_pkg_name);

    if dependent_file.exists() {
        fs::remove_file(dependent_file)?;
    }

    Ok(())
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
                        Command::new("cmd").arg("/C").arg(&final_cmd).output()?
                    } else {
                        Command::new("sh").arg("-c").arg(&final_cmd).output()?
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

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    let (pkg, _) = resolve::resolve_package_and_version(package_name)?;

    let (_manifest, scope) =
        if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::User)? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::System)? {
            (m, types::Scope::System)
        } else {
            return Err(format!("Package '{}' is not installed by Zoi.", pkg.name).into());
        };

    if pkg.package_type == types::PackageType::Config {
        config_handler::run_uninstall_commands(&pkg)?;
    }

    let dependents_dir = local::get_store_root(scope)?
        .join(&pkg.name)
        .join("dependents");
    if dependents_dir.exists() {
        let entries: Vec<String> = fs::read_dir(&dependents_dir)?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        if !entries.is_empty() {
            return Err(format!(
                "Cannot uninstall '{}' because other packages depend on it:\n  - {}\n\nPlease uninstall these packages first.",
                &pkg.name,
                entries.join("\n  - ")
            ).into());
        }
    }

    println!(
        "No packages depend on '{}'. Proceeding with uninstallation.",
        &pkg.name
    );

    println!("Cleaning up dependency records...");

    let cleanup = |dep_str: &String| -> Result<(), Box<dyn Error>> {
        if !dep_str.contains(':') || dep_str.starts_with("zoi:") {
            let dep_name = dep_str.split(['=', '>', '<', '~', '^']).next().unwrap();
            let final_dep_name = dep_name.strip_prefix("zoi:").unwrap_or(dep_name);
            remove_dependent_record(final_dep_name, &pkg.name, scope)?;
        }
        Ok(())
    };

    if let Some(ref deps) = pkg.dependencies {
        if let Some(ref runtime_deps) = deps.runtime {
            for dep_str in runtime_deps.get_required() {
                cleanup(dep_str)?;
            }
            for dep_str in runtime_deps.get_optional() {
                cleanup(dep_str)?;
            }
        }
        if let Some(ref build_deps) = deps.build {
            for dep_str in build_deps.get_required() {
                cleanup(dep_str)?;
            }
            for dep_str in build_deps.get_optional() {
                cleanup(dep_str)?;
            }
        }
    }

    let store_dir = local::get_store_root(scope)?.join(&pkg.name);
    if store_dir.exists() {
        println!("Removing stored files from {}...", store_dir.display());
        fs::remove_dir_all(&store_dir)?;
        println!("{}", "Successfully removed stored files.".green());
    } else {
        println!(
            "{} No stored files found (was already partially removed).",
            "Warning:".yellow()
        );
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

    Ok(())
}
