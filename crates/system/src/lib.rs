//! `ZoiOS` system-management primitives.
//!
//! This crate holds everything the privileged side of Zoi needs: declarative
//! account reconciliation, the kernel and bootloader pipeline, system
//! generations, the daemon protocol, and machine-local secrets.
//!
//! It is deliberately free of CLI concerns. The `zoi system` commands and the
//! `zoid` daemon are both thin layers over the functions here, which is what
//! lets the same code serve a live root and a sysroot being assembled during
//! `zoi system distro build`.
/// Declarative account reconciliation for `system.lua`.
pub mod account;
pub mod boot;
#[cfg(unix)]
/// Client side of the `zoid` daemon protocol.
pub mod client;
/// Parsed configuration.
pub mod config;
/// Bootstrapping a distribution onto a target root.
pub mod distro;
/// Early boot.
pub mod early_boot;
/// Generation this record belongs to.
pub mod generation;
/// Home directory path.
pub mod home;
pub mod kernel;
/// Mount point.
pub mod mount;
/// The `zoid` daemon protocol.
pub mod protocol;
/// Machine-local secret handling.
pub mod secret;
/// Enabling and disabling system services.
pub mod service;
