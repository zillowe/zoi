//! Logic for the `unpin` command.

use anyhow::Result;
use colored::Colorize;

use crate::pkg::{config, pin, resolve};

/// Run the unpin command.
///
/// Removes both halves of a pin created by `zoi pin`: the upgrade guard in
/// `pinned.json` and the preferred-version entries in the `versions:` map of
/// `config.yaml`.
///
/// # Errors
///
/// Returns an error if the package cannot be resolved or if updating the pin
/// configuration fails.
pub fn run(source: &str, yes: bool) -> Result<()> {
    let (pkg, _, _, _, _registry_handle, _, _) =
        resolve::resolve_package_and_version(source, None, false, yes)?;
    let mut pinned_packages = pin::get_pinned_packages()?;

    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.source != pkg.name);

    if pinned_packages.len() == initial_len {
        println!("Package '{}' was not pinned.", pkg.name);
        return Ok(());
    }

    pin::write_pinned_packages(&pinned_packages)?;

    // Drop the shim preference for this package's binaries, mirroring the keys
    // `zoi pin` wrote. Without this, an unpinned package would keep pointing
    // its shim at the old version forever.
    //
    // Only the exact keys this package owns are removed, so unpinning one
    // package never clobbers another package's preference for a shared binary
    // name.
    let mut config = config::read_user_config()?;
    let owned: Vec<String> = match &pkg.bins {
        Some(bins) if !bins.is_empty() => bins.clone(),
        // `zoi pin` falls back to the package name when it declares no bins,
        // so that is the only key that could have been written for it.
        _ => vec![pkg.name.clone()]
    };
    let mut changed = false;
    for key in owned {
        if config.versions.remove(&key).is_some() {
            changed = true;
        }
    }
    if changed {
        config::write_user_config(&config)?;
    }

    println!("Unpinned {}", pkg.name.green());
    Ok(())
}
