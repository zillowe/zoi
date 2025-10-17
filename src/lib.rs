//! Zoi: The Universal Package Manager & Environment Setup Tool.
//!
//! This crate provides the core functionality of Zoi as a library, allowing other
//! Rust applications to leverage its package management and environment setup
//! capabilities.

pub mod cli;
pub mod cmd;
pub mod pkg;
pub mod project;
pub mod utils;

use anyhow::Result;
pub use pkg::types::{self, Scope};
use std::path::Path;

/// Builds a Zoi package from a local .pkg.lua file.
pub fn build(
    package_file: &Path,
    build_type: &str,
    platforms: &[String],
    sign_key: Option<String>,
) -> Result<()> {
    pkg::package::build::run(package_file, build_type, platforms, sign_key, None)
}

/// Installs a Zoi package from a local package archive.
pub fn install_package(
    package_file: &Path,
    scope_override: Option<Scope>,
    registry_handle: &str,
) -> Result<Vec<String>> {
    pkg::package::install::run(package_file, scope_override, registry_handle)
}

/// Uninstalls a Zoi package.
pub fn uninstall_package(package_name: &str) -> Result<()> {
    pkg::uninstall::run(package_name)
}
