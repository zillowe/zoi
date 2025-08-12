pub mod cli;
pub mod cmd;
pub mod pkg;
mod project;
mod utils;

pub use pkg::install::InstallMode;
pub use pkg::types::InstallReason;
use std::collections::HashSet;
use std::error::Error;

/// Installs a Zoi package.
///
/// This is the main entry point for using Zoi's installation logic as a library.
///
/// # Arguments
///
/// * `source` - The package source identifier (e.g. "package-name", "@repo/package-name", "http://...", "/path/to/file.pkg.yaml").
/// * `mode` - The installation mode to use.
/// * `force` - If true, forces re-installation even if the package is already installed.
/// * `reason` - The reason for installation (direct user action or as a dependency).
/// * `yes` - If true, automatically confirms any prompts.
///
/// # Returns
///
/// `Ok(())` on success, or an error if installation fails.
///
/// # Example
///
/// ```no_run
/// use zoi::{install, InstallMode, InstallReason};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let source = "hello";
///     let mode = InstallMode::PreferBinary;
///     let force = false;
///     let reason = InstallReason::Direct;
///     let yes = true;
///
///     zoi::install(source, mode, force, reason, yes)?;
///
///     Ok(())
/// }
/// ```
pub fn install(
    source: &str,
    mode: InstallMode,
    force: bool,
    reason: InstallReason,
    yes: bool,
) -> Result<(), Box<dyn Error>> {
    // Internally, run_installation uses this to track dependencies and avoid cycles.
    // For a top-level install call, we start with an empty set.
    let mut processed_deps = HashSet::new();
    pkg::install::run_installation(source, mode, force, reason, yes, &mut processed_deps)
}
