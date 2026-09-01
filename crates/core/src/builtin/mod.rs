//! Built-in resources bundled with Zoi.
//!
//! This module exposes the pre-defined registries that ship with Zoi so that
//! handle-based registry management, PURL resolution, and Zoi Mini work out of
//! the box without relying on an external central database.

/// Loading and parsing of pre-defined built-in registries.
pub mod registry;

/// Pre-defined registries embedded at compile time.
pub mod registries {
    include!(concat!(env!("OUT_DIR"), "/generated_registries.rs"));
}
