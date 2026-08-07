//! Core logic and types for Zoi.
//!
//! This crate contains the foundational building blocks for Zoi, including:
//! - Configuration management and policy enforcement.
//! - Package and dependency models.
//! - PGP signature verification.
//! - Hash calculations and cache management.
//! - Sysroot and path utilities.

/// Built-in package definitions and resources.
pub mod builtin;
/// Cache management for archives and package definitions.
pub mod cache;
/// Configuration management and policy enforcement.
pub mod config;
/// Dependency resolution and management.
pub mod dependency;
/// Management of frozen/locked package states.
pub mod frozen;
/// Hash calculation and verification.
pub mod hash;
/// Lock file management.
pub mod lock;
/// Offline mode support and utilities.
pub mod offline;
/// PGP signature verification.
pub mod pgp;
/// Package pinning management.
pub mod pin;
/// Package directory management.
pub mod pkgdir;
/// Transaction recording and playback.
pub mod recorder;
/// Sysroot and installation path management.
pub mod sysroot;
/// Core types used across the codebase.
pub mod types;
/// Package upgrade logic.
pub mod upgrade;
/// General utility functions.
pub mod utils;
