use crate::pkg::types;
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::thread;

use std::path::PathBuf;

pub fn build_archive(
    pkg_lua_path: &std::path::Path,
    pkg: &types::Package,
    build_type_override: Option<&str>,
    pb: Option<&indicatif::ProgressBar>,
) -> Result<PathBuf> {
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

    if let Some(p) = pb {
        p.set_message("Building package...");
        p.set_position(0);
    } else {
        println!("Building {}...", pkg.name.cyan());
    }

    let pkg_lua_path_clone = pkg_lua_path.to_path_buf();
    let build_type_clone = build_type.to_string();
    let current_platform_clone = current_platform.clone();
    let version_clone = version.to_string();

    let build_handle = thread::spawn(move || {
        crate::pkg::package::build::run(
            &pkg_lua_path_clone,
            Some(&build_type_clone),
            std::slice::from_ref(&current_platform_clone),
            None,
            None,
            Some(&version_clone),
            None,
            true,
            true,
            "native",
            None,
        )
    });

    let build_result = build_handle
        .join()
        .map_err(|_| anyhow!("Build thread panicked"))?;

    if let Err(e) = build_result {
        if let Some(p) = pb {
            p.finish_with_message(format!("{}", "Build failed".red()));
        }
        return Err(anyhow!(
            "'build' step failed: {}\nEnable verbose logging with -v to see more details.",
            e
        ));
    }

    let archive_filename = format!("{}-{}-{}.pkg.tar.zst", pkg.name, version, current_platform);
    let archive_path = pkg_lua_path
        .parent()
        .expect("pkg_lua_path should have a parent")
        .join(archive_filename);
    if !archive_path.exists() {
        return Err(anyhow!(
            "Package archive '{}' was not created after a successful build. This is an unexpected error.",
            archive_path.display()
        ));
    }

    if let Some(p) = pb {
        p.set_position(100);
    } else {
        println!("Finished building {}.", pkg.name.cyan());
    }

    Ok(archive_path)
}
