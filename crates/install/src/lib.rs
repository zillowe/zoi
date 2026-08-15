//! Zoi installation logic.
//!
//! This crate contains the logic for installing packages, managing lockfiles,
//! and resolving dependencies.

/// Module for creating package installations.
pub mod create;
/// Module for installing dependencies.
pub mod dep_install;
/// Module for the main installer logic.
pub mod installer;
/// Module for managing lockfiles.
pub mod lockfile;
/// Module for package manifests.
pub mod manifest;
/// Module for package installation logic.
pub mod pkg_install;
/// Module for installation plans.
pub mod plan;
/// Module for handling prebuilt packages.
pub mod prebuilt;
/// Module for the `PubGrub` dependency resolver.
pub mod pubgrub;
/// Module for resolving dependencies.
pub mod resolver;
/// Module for managing services.
pub mod service;
/// Module for creating shims.
pub mod shim;
/// Utility functions.
pub mod util;
