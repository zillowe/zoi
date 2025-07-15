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

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    if local::is_package_installed(package_name)?.is_none() {
        return Err(format!(
            "Package '{}' is not installed by Zoi. Cannot uninstall.",
            package_name
        )
        .into());
    }

    let store_dir = get_store_root()?.join(package_name);
    let symlink_path = get_bin_root()?.join(package_name);

    if store_dir.exists() {
        println!("  Removing stored files from {}...", store_dir.display());
        fs::remove_dir_all(&store_dir)?;
        println!("{}", "  Successfully removed stored files.".green());
    } else {
        println!(
            "{} No stored files found (was already partially removed).",
            "Warning:".yellow()
        );
    }

    if symlink_path.exists() {
        println!("  Removing symlink from {}...", symlink_path.display());
        fs::remove_file(&symlink_path)?;
        println!("{}", "  Successfully removed symlink.".green());
    }

    Ok(())
}
