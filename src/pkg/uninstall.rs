use crate::pkg::{config_handler, local, resolve, types};
use colored::*;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

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
                &pkg.name, entries.join("\n  - ")
            ).into());
        }
    }

    println!("No packages depend on '{}'. Proceeding with uninstallation.", &pkg.name);

    println!("Cleaning up dependency records...");

    let cleanup = |dep_str: &String| -> Result<(), Box<dyn Error>> {
        if !dep_str.contains(':') || dep_str.starts_with("zoi:") {
            let dep_name = dep_str.split(['=', '>', '<', '~', '^']).next().unwrap();
            let final_dep_name = dep_name.strip_prefix("zoi:").unwrap_or(dep_name);
            remove_dependent_record(final_dep_name, &pkg.name, scope)?;
        }
        Ok(())
    };

    if let Some(deps) = pkg.dependencies {
        if let Some(runtime_deps) = deps.runtime {
            for dep_str in runtime_deps.get_required() {
                cleanup(dep_str)?;
            }
            for dep_str in runtime_deps.get_optional() {
                cleanup(dep_str)?;
            }
        }
        if let Some(build_deps) = deps.build {
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

    Ok(())
}

