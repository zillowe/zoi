//! Project management and configuration for Zoi.
//!
//! This crate handles project-level operations, including loading
//! configurations, managing environments, executing tasks, and handling
//! lockfiles.

/// Project configuration loading and structures.
pub mod config;
/// Environment setup and management.
pub mod environment;
/// Command execution logic.
pub mod executor;
/// Lockfile management and frozen package definitions.
pub mod lockfile;
/// Lua-based configuration parsing.
pub mod lua_config;
/// Task runner and dependency resolution.
pub mod runner;
/// Project verification and health checks.
pub mod verify;
