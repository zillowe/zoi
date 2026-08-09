//! Installation of package archive files.

use crate::cli::SetupScope;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Command to install a package from a `.zpa` archive file.
#[derive(Parser, Debug)]
pub struct InstallCommand {
    /// Path to the package archive file (e.g. path/to/name-os-arch.zpa)
    #[arg(required = true)]
    pub package_file: PathBuf,
    /// The sub-packages to install from the archive.
    #[arg(long, short, num_args = 1..)]
    pub sub: Option<Vec<String>>,
    /// The scope to install the package to (user or system-wide)
    #[arg(long, value_enum, default_value_t = SetupScope::User)]
    pub scope: SetupScope,
    /// Automatically answer yes to all prompts
    #[arg(long)]
    pub yes: bool,
}

/// Runs the package installation command.
///
/// This installs a package from a local `.zpa` archive into the specified scope.
///
/// # Errors
///
/// Returns an error if the package file cannot be read, if dependencies cannot be
/// resolved, or if the installation fails.
pub fn run(args: InstallCommand) -> Result<()> {
    let scope = match args.scope {
        SetupScope::User => crate::pkg::types::Scope::User,
        SetupScope::System => crate::pkg::types::Scope::System,
    };
    crate::pkg::install::pkg_install::run(
        &args.package_file,
        Some(scope),
        "local",
        None,
        args.yes,
        args.sub,
        true,
        None,
    )?;
    Ok(())
}
