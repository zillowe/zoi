//! Pinning packages to specific versions.

use std::collections::HashMap;

use anyhow::Result;
use colored::Colorize;

use crate::pkg::{config, pin, resolve};

/// Pins a package to a specific version.
///
/// This prevents the package from being updated to a newer version during
/// general upgrades, and records the version as the preferred global version
/// for the binaries the package provides.
///
/// Pinning touches two different stores, because they answer different
/// questions:
/// - `pinned.json` answers "may this package be upgraded?", which is what keeps
///   a general `zoi update` from moving it off the pinned version.
/// - The `versions:` map in `config.yaml` answers "which version should the
///   shim launch?", and is the last fallback in shim resolution after the env
///   var, plugin hook, `zoi.lua` and `.tool-versions` are consulted.
///
/// Pinning both keeps the two consistent. Setting only the first would let the
/// package stay installed at the old version while the shim still preferred a
/// newer one.
///
/// # Errors
///
/// Returns an error if the package cannot be resolved, or if the pinned
/// packages file or the user config cannot be read or written.
pub fn run(source: &str, version: &str, yes: bool) -> Result<()> {
    let (pkg, _, _, _, _registry_handle, _, _) =
        resolve::resolve_package_and_version(source, None, false, yes)?;
    let mut pinned_packages = pin::get_pinned_packages()?;

    if pinned_packages.iter().any(|p| p.source == pkg.name) {
        println!(
            "Package '{}' is already pinned. Unpin it first to change the \
             version.",
            pkg.name
        );
        return Ok(());
    }

    let new_pin = pin::PinnedPackage {
        source: pkg.name.clone(),
        version: version.to_string()
    };
    pinned_packages.push(new_pin);
    pin::write_pinned_packages(&pinned_packages)?;

    // Record the preferred version for every binary the package provides. The
    // map is keyed by binary name because that is what the shim looks up. A
    // package with no declared `bins` still gets an entry under its own name,
    // which covers the common case where the binary and the package share a
    // name.
    let mut versions: HashMap<String, String> = pkg
        .bins
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|bin| (bin, version.to_string()))
        .collect();
    if versions.is_empty() {
        versions.insert(pkg.name.clone(), version.to_string());
    }
    config::update_global_versions(versions)?;

    println!("Pinned {}@{}", pkg.name.green(), version.yellow());
    Ok(())
}
