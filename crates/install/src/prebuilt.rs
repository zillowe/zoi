use std::path::PathBuf;
use std::thread;

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::{types, utils};
use zoi_deps as dependencies;

use crate::dep_install;

/// Builds a package archive from source.
///
/// This function:
/// - Resolves the appropriate build type for the current platform.
/// - Resolves and installs any required build-time dependencies.
/// - Spawns a build thread to execute the build process defined in the
///   `.pkg.lua`.
/// - Returns the path to the resulting `.zpa` (Zoi Package Archive) file.
/// # Errors
///
/// Returns an error if the build fails, or if build-time dependencies cannot be
/// resolved.
pub fn build_archive(
    pkg_lua_path: &std::path::Path,
    pkg: &types::Package,
    sub_package: Option<&str>,
    build_type_override: Option<&str>,
    pb: Option<&indicatif::ProgressBar>,
    quiet: bool
) -> Result<Option<PathBuf>> {
    let Some(build_type) = zoi_package::build::resolve_build_type(
        build_type_override,
        &pkg.types,
        &pkg.name
    )?
    else {
        if let Some(p) = pb {
            p.finish_with_message(format!(
                "{} Skipping build for '{}': no build types supported.",
                "::".bold().yellow(),
                pkg.name
            ));
        } else if !quiet {
            println!(
                "{} Skipping build for '{}': no build types supported.",
                "::".bold().yellow(),
                pkg.name
            );
        }
        return Ok(None);
    };

    let current_platform = utils::get_platform()?;
    let version = pkg.version.as_deref().ok_or_else(|| {
        anyhow!(
            "Version not resolved for build for package '{}'. This is an \
             internal error.",
            pkg.name
        )
    })?;

    let display_name = if let Some(sub) = sub_package {
        format!("{}:{}", pkg.name, sub)
    } else {
        pkg.name.clone()
    };

    if let Some(p) = pb {
        p.set_message(format!("Building {}...", display_name.cyan()));
        p.set_position(0);
    }

    if !quiet {
        println!("Building {}...", display_name.cyan());
    }

    let mut all_build_deps = Vec::new();

    if let Some(dep_strings) = zoi_package::build::get_build_dependencies(
        pkg_lua_path,
        Some(&build_type),
        &current_platform,
        Some(version),
        quiet
    )? {
        all_build_deps.extend(dep_strings);
    }

    if !all_build_deps.is_empty() {
        if let Some(p) = pb {
            p.set_message(format!(
                "Installing build deps for {}...",
                display_name.cyan()
            ));
        } else if !quiet {
            println!(
                "Installing build dependencies for {}...",
                display_name.cyan()
            );
        }
        let processed = std::sync::Mutex::new(std::collections::HashSet::new());
        let mut installed = Vec::new();
        for dep_str in all_build_deps {
            let dep = dependencies::parse_dependency_string(&dep_str)?;
            dep_install::install_dependency(
                &dep,
                &pkg.name,
                pkg.scope,
                true,
                true,
                &processed,
                &mut installed,
                None
            )?;
        }
    }

    if let Some(p) = pb {
        p.set_message(format!("Building {}...", display_name.cyan()));
    }

    let pkg_lua_path_clone = pkg_lua_path.to_path_buf();
    let build_type_clone = build_type.clone();
    let current_platform_clone = current_platform.clone();
    let version_clone = version.to_string();
    let sub_packages = sub_package.map(|s| vec![s.to_string()]);

    let build_handle = thread::spawn(move || {
        zoi_package::build::run(
            &pkg_lua_path_clone,
            Some(&build_type_clone),
            std::slice::from_ref(&current_platform_clone),
            None,
            zoi_core::types::SignMode::Embed,
            None,
            Some(&version_clone),
            sub_packages,
            quiet,
            "native",
            None,
            false,
            false,
            false
        )
    });

    let build_result = build_handle.join().map_err(|_| {
        anyhow!("Build thread panicked for package '{}'", pkg.name)
    })?;

    if let Err(e) = build_result {
        if let Some(p) = pb {
            p.finish_with_message(format!(
                "{}: {}",
                pkg.name.cyan(),
                "Build failed".red()
            ));
        }
        return Err(anyhow!(
            "Build failed for package '{}': {}\nEnable verbose logging with \
             -v to see more details.",
            pkg.name,
            e
        ));
    }

    let archive_filename =
        format!("{}-{}-{}.zpa", pkg.name, version, current_platform);
    let archive_path = pkg_lua_path
        .parent()
        .ok_or_else(|| {
            anyhow!(
                "pkg_lua_path should have a parent: {}",
                pkg_lua_path.display()
            )
        })?
        .join(archive_filename);
    if !archive_path.exists() {
        return Err(anyhow!(
            "Package archive '{}' was not created after a successful build. \
             This is an unexpected error.",
            archive_path.display()
        ));
    }

    if let Some(p) = pb {
        p.set_position(100);
    } else if !quiet {
        println!("Finished building {}.", pkg.name.cyan());
    }

    Ok(Some(archive_path))
}
