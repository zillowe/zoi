//! Lua APIs for Zoi.
//!
//! This module contains the implementations of the various APIs exposed to
//! Lua scripts, organized into submodules by functionality.

/// Archive manipulation utilities (zip, tar, etc.).
pub mod archive;
/// Cryptographic utilities (hash verification, PGP).
pub mod crypto;
/// File download utilities with progress reporting.
pub mod download;
/// Filesystem and staging utilities.
pub mod fs;
/// HTTP and Git-forge integration.
pub mod http;
/// Core Package DSL and lifecycle functions.
pub mod lifecycle;
/// Data parsing utilities (JSON, YAML, TOML).
pub mod parse;
/// System command execution and patching.
pub mod system;
