//! Dependency resolution and package lookup for Zoi.
//!
//! This crate provides the logic for resolving package requests to concrete
//! installation sources, searching local and remote registries, and managing
//! installed package manifests.

pub mod local;
pub mod mini_resolve;
pub mod resolve;
