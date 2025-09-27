use crate::pkg::types;
use crate::utils;
use anyhow::Result;
use colored::*;
use std::error::Error;
use std::fs;

pub fn try_meta_build_install(
    pkg_lua_path: &std::path::Path,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    println!(
        "{}",
        "Attempting to build and install from meta files...".yellow()
    );

    if let Err(e) = crate::pkg::package::meta::run(pkg_lua_path, None, pkg.version.as_deref()) {
        return Err(format!("'meta' step failed: {}", e).into());
    }
    let meta_filename = format!("{}.meta.json", pkg.name);
    let meta_path = pkg_lua_path.with_file_name(meta_filename);
    if !meta_path.exists() {
        return Err("meta.json file not created".into());
    }
    println!("'meta' step successful.");

    let current_platform = utils::get_platform()?;
    if let Err(e) = crate::pkg::package::build::run(&meta_path, &[current_platform]) {
        return Err(format!("'build' step failed: {}", e).into());
    }
    let platform = utils::get_platform()?;
    let archive_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        pkg.name,
        pkg.version.as_deref().unwrap_or(""),
        platform
    );
    let archive_path = meta_path.with_file_name(archive_filename);
    if !archive_path.exists() {
        return Err("package archive not created".into());
    }
    println!("'build' step successful.");

    if let Err(e) = crate::pkg::package::install::run(&archive_path, Some(pkg.scope)) {
        return Err(format!("'install' step failed: {}", e).into());
    }
    println!("'install' step successful.");

    let _ = fs::remove_file(&meta_path);
    let _ = fs::remove_file(&archive_path);

    Ok(())
}
