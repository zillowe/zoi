use crate::pkg::local;
use colored::*;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn get_store_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("store"))
}

fn get_bin_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("bin"))
}

fn remove_dependent_record(
    dependency_name: &str,
    dependent_pkg_name: &str,
) -> Result<(), Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let dependent_file = home_dir
        .join(".zoi/pkgs/store")
        .join(dependency_name)
        .join("dependents")
        .join(dependent_pkg_name);

    if dependent_file.exists() {
        fs::remove_file(dependent_file)?;
    }

    Ok(())
}

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    let manifest = local::is_package_installed(package_name)?
        .ok_or_else(|| format!("Package '{package_name}' is not installed by Zoi."))?;

    let dependents_dir = get_store_root()?.join(package_name).join("dependents");
    if dependents_dir.exists() {
        let entries: Vec<String> = fs::read_dir(&dependents_dir)?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        if !entries.is_empty() {
            return Err(format!(
                "Cannot uninstall '{}' because other packages depend on it:\n  - {}\n\nPlease uninstall these packages first.",
                package_name, entries.join("\n  - ")
            ).into());
        }
    }

    println!("No packages depend on '{package_name}'. Proceeding with uninstallation.");

    println!("Cleaning up dependency records...");
    let pkg_path = crate::pkg::resolve::resolve_source(&manifest.name)?;
    let content = fs::read_to_string(pkg_path.path)?;
    let pkg_def: crate::pkg::types::Package = serde_yaml::from_str(&content)?;

    let cleanup = |dep_str: &String| -> Result<(), Box<dyn Error>> {
        if !dep_str.contains(':') || dep_str.starts_with("zoi:") {
            let dep_name = dep_str.split(['=', '>', '<', '~', '^']).next().unwrap();
            let final_dep_name = dep_name.strip_prefix("zoi:").unwrap_or(dep_name);
            remove_dependent_record(final_dep_name, package_name)?;
        }
        Ok(())
    };

    if let Some(deps) = pkg_def.dependencies {
        if let Some(runtime_deps) = deps.runtime {
            for dep_str in &runtime_deps {
                cleanup(dep_str)?;
            }
        }
        if let Some(build_deps) = deps.build {
            for dep_str in &build_deps {
                cleanup(dep_str)?;
            }
        }
    }

    let store_dir = get_store_root()?.join(package_name);
    if store_dir.exists() {
        println!("Removing stored files from {}...", store_dir.display());
        fs::remove_dir_all(&store_dir)?;
        println!("{}", "Successfully removed stored files.".green());
    } else {
        println!("{} No stored files found (was already partially removed).", "Warning:".yellow());
    }

    let symlink_path = get_bin_root()?.join(package_name);
    if symlink_path.exists() {
        println!("Removing symlink from {}...", symlink_path.display());
        fs::remove_file(&symlink_path)?;
        println!("{}", "Successfully removed symlink.".green());
    }

    Ok(())
}
