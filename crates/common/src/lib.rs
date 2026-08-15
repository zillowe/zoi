//! Common types and utilities for the Zoi package manager.
//!
//! This crate provides shared data structures, traits, and user experience (UX)
//! components that are used across multiple Zoi crates, including the main
//! CLI and Zoi Mini.

use anyhow::Result;
use zoi_core::types::Scope;

/// User experience data structures and utilities.
pub mod ux;

/// Options for installing packages from source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SourceInstallOptions {
    /// The repository to install from.
    pub repo: Option<String>,
    /// Whether to force the installation.
    pub force: bool,
    /// Whether to install all optional dependencies.
    pub all_optional: bool,
    /// Whether to skip confirmation prompts.
    pub yes: bool,
    /// Override the installation scope.
    pub scope_override: Option<Scope>,
    /// Whether to save the installation to the project file.
    pub save: bool,
    /// The build type to use.
    pub build_type: Option<String>,
    /// Whether to perform a dry run.
    pub dry_run: bool,
    /// Whether to build the package.
    pub build: bool,
    /// Whether to use the lockfile exactly (frozen).
    pub frozen: bool
}

/// A trait for subcommands that can be executed.
pub trait Runnable {
    /// Executes the subcommand.
    ///
    /// # Errors
    ///
    /// Returns an error if the command execution fails.
    fn run(&self, yes: bool) -> Result<()>;
}
