//! Loading and parsing of pre-defined built-in registries.
//!
//! The registries themselves live as YAML files in
//! `src/builtin/registries/<handle>.yaml` and are embedded into the binary at
//! compile time (see `crates/core/build.rs`). This module turns those embedded
//! YAML strings into `BuiltinRegistry` values and provides lookup helpers.

use super::registries::BUILTIN_REGISTRIES;
use crate::types::BuiltinRegistry;

/// Loads all pre-defined built-in registries.
///
/// Registries whose YAML cannot be parsed are skipped. This should never happen
/// in practice because the builds are validated, but failing one registry must
/// not break the whole tool.
pub fn load_all() -> Vec<BuiltinRegistry> {
    BUILTIN_REGISTRIES
        .iter()
        .filter_map(|(handle, raw)| {
            serde_yaml::from_str::<BuiltinRegistry>(raw)
                .ok()
                .or_else(|| {
                    eprintln!(
                        "Warning: failed to parse built-in registry '{handle}'"
                    );
                    None
                })
        })
        .collect()
}

/// Looks up a built-in registry by its handle.
pub fn get(handle: &str) -> Option<BuiltinRegistry> {
    load_all().into_iter().find(|r| r.handle == handle)
}

/// Returns the single built-in registry marked as `set` (default).
///
/// # Errors
///
/// Returns an error if more than one built-in registry is marked as the `set`
/// (default) registry, which is an invalid state.
///
/// # Panics
///
/// Panics if the set of `set` registries is internally inconsistent (guaranteed
/// not to happen because this is only reached when exactly one registry
/// matched).
pub fn get_set() -> anyhow::Result<Option<BuiltinRegistry>> {
    let set: Vec<BuiltinRegistry> =
        load_all().into_iter().filter(|r| r.set).collect();

    match set.len() {
        0 => Ok(None),
        1 => Ok(Some(set.into_iter().next().expect("length checked"))),
        _ => Err(anyhow::anyhow!(
            "More than one built-in registry is marked as the set registry \
             ({}). Only one registry can be the official set registry. Please \
             fix the built-in registries definitions.",
            set.iter()
                .map(|r| r.handle.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}
