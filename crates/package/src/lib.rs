//! Zoi package management, building, and registry operations.
//!
//! This crate provides the core logic for creating, building, and managing Zoi packages
//! and registries. It handles the lifecycle of a package from its `.pkg.lua` definition
//! to a distributable `.zpa` or `.zsa` archive.

/// Package build orchestration.
pub mod build;
/// Source bundling into `.zsa` archives.
pub mod bundle;
/// Linux isolation using Bubblewrap.
pub mod bwrap;
/// Containerized builds using Docker.
pub mod docker;
/// Package health and metadata validation.
pub mod doctor;
/// System-wide environment health checks.
pub mod doctor_system;
/// LSP support for package development.
pub mod init_lsp;
/// File pooling and deduplication for package archives.
pub mod pool;
/// Registry management and metadata generation.
pub mod registry;
/// ELF binary relocation for portability.
pub mod relocate;
