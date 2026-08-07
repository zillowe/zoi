//! Lua scripting and API integration for Zoi.
//!
//! This crate provides the Lua engine and the core APIs exposed to Zoi
//! package definitions (`.pkg.lua`) and plugins. It handles the execution
//! of Lua scripts and provides a bridge between Lua and Zoi's Rust core.

/// Core Lua API implementations.
pub mod api;
/// High-level Lua environment setup and execution functions.
pub mod functions;
/// Lua script parser and validator.
pub mod parser;
