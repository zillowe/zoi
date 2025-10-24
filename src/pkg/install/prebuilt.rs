use crate::pkg::types;
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::fs;

pub fn try_build_install(
    pkg_lua_path: &std::path::Path,
    pkg: &types::Package,
    registry_handle: &str,
    build_type_override: Option<&str>,
    yes: bool,
) -> Result<Vec<String>> {
    println!("{}", "Attempting to build and install package...".yellow());

    let build_type = if let Some(t) = build_type_override {
        if !pkg.types.contains(&t.to_string()) {
            return Err(anyhow!(
                "Build type '{}' not supported by this package. Supported types: {:?}",
                t,
                pkg.types
            ));
        }
        t
    } else if pkg.types.contains(&"pre-compiled".to_string()) {
        "pre-compiled"
    } else if !pkg.types.is_empty() {
        &pkg.types[0]
    } else {
        return Err(anyhow!(
            "No supported build types found in package '{}'. Please specify a `types` field in the package file (e.g. `types = {{ 'source' }}`).",
            pkg.name
        ));
    };

    let current_platform = utils::get_platform()?;
    let version = pkg.version.as_deref().ok_or_else(|| {
        anyhow!(
            "Version not resolved for build for package '{}'. This is an internal error.",
            pkg.name
        )
    })?;
    if let Err(e) = crate::pkg::package::build::run(
        pkg_lua_path,
        build_type,
        std::slice::from_ref(&current_platform),
        None,
        None,
        Some(version),
    ) {
        return Err(anyhow!("'build' step failed: {}", e));
    }

    let archive_filename = format!(
        "{}-{}-{}.pkg.tar.zst",
        pkg.name,
        pkg.version.as_deref().unwrap_or(""),
        current_platform
    );
    let archive_path = pkg_lua_path.parent().unwrap().join(archive_filename);
    if !archive_path.exists() {
        return Err(anyhow!(
            "Package archive '{}' was not created after a successful build. This is an unexpected error.",
            archive_path.display()
        ));
    }
    println!("'build' step successful.");

    let installed_files = crate::pkg::package::install::run(
        &archive_path,
        Some(pkg.scope),
        registry_handle,
        Some(version),
        yes,
    )
    .map_err(|e| anyhow!("Failed to install built package archive: {}", e))?;
    println!("'install' step successful.");

    let _ = fs::remove_file(&archive_path);

    Ok(installed_files)
}
