use crate::pkg::types;
use crate::utils;
use anyhow::Result;
use colored::*;
use std::error::Error;
use std::fs;

pub fn try_build_install(
    pkg_lua_path: &std::path::Path,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "Attempting to build and install package...".yellow());

    let build_type = if pkg.types.contains(&"pre-compiled".to_string()) {
        "pre-compiled"
    } else if pkg.types.contains(&"source".to_string()) {
        "source"
    } else {
        return Err("No supported build types found in package".into());
    };

    let current_platform = utils::get_platform()?;
    if let Err(e) = crate::pkg::package::build::run(
        pkg_lua_path,
        build_type,
        std::slice::from_ref(&current_platform),
    ) {
        return Err(format!("'build' step failed: {}", e).into());
    }

    let archive_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        pkg.name,
        pkg.version.as_deref().unwrap_or(""),
        current_platform
    );
    let archive_path = pkg_lua_path.parent().unwrap().join(archive_filename);
    if !archive_path.exists() {
        return Err("package archive not created".into());
    }
    println!("'build' step successful.");

    if let Err(e) = crate::pkg::package::install::run(&archive_path, Some(pkg.scope)) {
        return Err(format!("'install' step failed: {}", e).into());
    }
    println!("'install' step successful.");

    let _ = fs::remove_file(&archive_path);

    Ok(())
}
