pub mod cli;
pub mod cmd;
pub mod pkg;
mod project;
mod utils;

pub use pkg::install::InstallMode;
pub use pkg::types::InstallReason;
pub use pkg::update::UpdateResult;
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
    pkg::install::run_installation(source, mode, force, reason, yes, false, &mut processed_deps)
}

/// Updates a Zoi package if a new version is available.
///
/// This function checks the installed version of a package against the latest
/// available version. If they differ, it performs an update.
///
/// # Arguments
///
/// * `source` - The package source identifier (e.g. "package-name").
/// * `yes` - If true, automatically confirms any prompts.
///
/// # Returns
///
/// A `Result` containing an `UpdateResult` enum on success, or an error if the
/// update fails.
///
/// # Example
///
/// ```no_run
/// use zoi::{update, UpdateResult};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let source = "hello";
///     match zoi::update(source, true)? {
///         UpdateResult::Updated { from, to } => {
///             println!("Updated from {} to {}", from, to);
///         }
///         UpdateResult::AlreadyUpToDate => {
///             println!("Already up to date.");
///         }
///         UpdateResult::Pinned => {
///             println!("Package is pinned, skipping update.");
///         }
///     }
///     Ok(())
/// }
/// ```
pub fn update(source: &str, yes: bool) -> Result<pkg::update::UpdateResult, Box<dyn Error>> {
    pkg::update::run(source, yes)
}
