//! Implementation of the `owner` command, which finds the package that owns a
//! given file.

use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::pkg::local;

/// Runs the owner command.
///
/// # Errors
///
/// Returns an error if the path does not exist, or if there is an error
/// querying the package database or ownership information.
pub fn run(path: &Path) -> Result<()> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };

    println!("Querying for file: {}", absolute_path.display());

    let installed_packages = local::get_installed_packages()?;
    let query_canonical = path.canonicalize().ok();

    for pkg in installed_packages {
        let display_name = if let Some(sub) = &pkg.sub_package {
            format!("{}:{}", pkg.name, sub)
        } else {
            pkg.name.clone()
        };

        // Check if query matches a binary shim registered by this package
        if let Some(bins) = &pkg.bins {
            let bin_root = match pkg.scope {
                zoi_core::types::Scope::User => {
                    zoi_core::utils::get_user_bin_dir().ok()
                }
                zoi_core::types::Scope::System => {
                    Some(zoi_core::utils::get_system_bin_dir())
                }
                zoi_core::types::Scope::Project => std::env::current_dir()
                    .ok()
                    .map(|c| c.join(".zoi").join("pkgs").join("bin"))
            };

            if let Some(bin_root) = bin_root {
                for bin in bins {
                    let shim_path = bin_root.join(bin);
                    if shim_path == path || shim_path == absolute_path {
                        println!(
                            "{} is owned by {} {}",
                            absolute_path.display(),
                            display_name.cyan(),
                            pkg.version.yellow()
                        );
                        return Ok(());
                    }
                }
            }
        }

        let version_dir = local::get_package_version_dir(
            pkg.scope,
            &pkg.registry_handle,
            &pkg.repo,
            &pkg.name,
            &pkg.version
        )
        .unwrap_or_default();

        for file_pattern in &pkg.installed_files {
            let Ok(expanded) = crate::pkg::utils::expand_placeholders(
                file_pattern,
                &version_dir,
                pkg.scope
            ) else {
                continue;
            };

            let expanded_path = Path::new(&expanded);
            let expanded_canonical = expanded_path.canonicalize().ok();

            let is_match = expanded_path == path
                || expanded_path == absolute_path
                || (query_canonical.is_some()
                    && query_canonical.as_deref() == Some(expanded_path))
                || (expanded_canonical.is_some()
                    && expanded_canonical.as_deref() == Some(&absolute_path))
                || (query_canonical.is_some()
                    && expanded_canonical.is_some()
                    && query_canonical == expanded_canonical);

            if is_match {
                println!(
                    "{} is owned by {} {}",
                    absolute_path.display(),
                    display_name.cyan(),
                    pkg.version.yellow()
                );
                return Ok(());
            }
        }
    }

    println!("No package owns file: {}", absolute_path.display());
    Ok(())
}
