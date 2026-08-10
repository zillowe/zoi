//! Pinning packages to specific versions.

use anyhow::Result;
use colored::Colorize;

use crate::pkg::{pin, resolve};

/// Pins a package to a specific version.
///
/// This prevents the package from being updated to a newer version during
/// general upgrades.
///
/// # Errors
///
/// Returns an error if the package cannot be resolved or if the pinned packages
/// file cannot be read or written.
pub fn run(source: &str, version: &str) -> Result<()> {
    let (pkg, _, _, _, _registry_handle, _, _) =
        resolve::resolve_package_and_version(source, None, false, false)?;
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

    println!("Pinned {}@{}", pkg.name.green(), version.yellow());
    Ok(())
}
