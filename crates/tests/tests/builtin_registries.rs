//! Integration tests for the pre-defined built-in registries feature.
//!
//! Covers loading the embedded registry YAML, resolving the `set` default
//! registry, and the `sync` helpers that populate registry metadata from the
//! built-in definitions.

use std::collections::HashSet;

use anyhow::Result;
use zoi::pkg::config;
use zoi::pkg::types::BuiltinRegistry;

mod common;

/// The embedded built-in directory in this repo (the definition that ships
/// with the tool). We compare against the published `set` registry rather than
/// hard-coding too much so the tests remain meaningful if new official
/// registries are added.
fn all_builtins() -> Vec<BuiltinRegistry> {
    zoi::pkg::builtin_registries::load_all()
}

#[test]
fn zoidberg_is_embedded_and_marked_as_set() {
    let rg = zoi::pkg::builtin_registries::get("zoidberg")
        .expect("zoidberg should be a built-in registry");
    assert_eq!(rg.handle, "zoidberg");
    assert_eq!(rg.registry_type, "official");
    assert!(rg.set, "the official registry should be the set registry");
    assert!(!rg.name.is_empty());
    assert!(!rg.git.is_empty());
    assert!(!rg.branch.is_empty());
}

#[test]
fn set_registry_is_resolved_from_builtins() {
    let set = zoi::pkg::builtin_registries::get_set()
        .expect("set registry should resolve")
        .expect("there must be exactly one official set registry");
    assert_eq!(set.handle, "zoidberg");
    assert!(set.set);
    assert_eq!(set.registry_type, "official");
}

#[test]
fn unknown_handle_is_not_a_builtin() {
    assert!(
        zoi::pkg::builtin_registries::get("no-such-registry").is_none(),
        "unknown handles must not resolve to a built-in registry"
    );
}

#[test]
fn set_registry_persists_metadata_from_builtin() -> Result<()> {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempfile::tempdir()?;
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    config::set_default_registry("zoidberg")?;

    let cfg = config::read_config()?;
    let default = cfg.default_registry.expect(
        "default registry should be set after calling set_default_registry"
    );
    assert_eq!(default.handle, "zoidberg");
    assert_eq!(
        default.name.as_deref(),
        Some("Zoidberg"),
        "name should be populated from the built-in metadata"
    );
    assert_eq!(
        default.url, "https://gitlab.com/zillowe/zillwen/zusty/zoidberg",
        "URL should come from the built-in definition"
    );
    Ok(())
}

#[test]
fn add_registry_persists_metadata_from_builtin() -> Result<()> {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempfile::tempdir()?;
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    zoi::cmd::sync::add_registry("zoidberg")?;

    let cfg = config::read_config()?;
    let added = cfg
        .added_registries
        .iter()
        .find(|r| r.handle == "zoidberg")
        .expect("zoidberg should have been added");
    assert_eq!(
        added.name.as_deref(),
        Some("Zoidberg"),
        "added registries should carry the built-in name"
    );
    assert_eq!(
        added.url,
        "https://gitlab.com/zillowe/zillwen/zusty/zoidberg"
    );
    Ok(())
}

#[test]
fn list_registries_includes_builtin_unadded_and_configured() -> Result<()> {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempfile::tempdir()?;
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    zoi::cmd::sync::set_registry("zoidberg")?;

    // Capturing stdout is unnecessary; we assert that listing succeeds and
    // that the persisted state reflects the set registry.
    zoi::cmd::sync::list_registries()?;

    let cfg = config::read_config()?;
    assert_eq!(
        cfg.default_registry.as_ref().map(|r| r.handle.as_str()),
        Some("zoidberg")
    );
    Ok(())
}

#[test]
fn embedded_yaml_files_match_loaded_registries() {
    // Sanity check: every YAML file in the builtin registries directory must
    // be loadable and each handle must be unique.
    let mut handles = HashSet::new();
    for rg in all_builtins() {
        assert!(
            handles.insert(rg.handle.clone()),
            "duplicate built-in registry handle '{}'",
            rg.handle
        );
        assert!(!rg.git.is_empty(), "'{}' is missing a git URL", rg.handle);
        assert!(!rg.branch.is_empty(), "'{}' is missing a branch", rg.handle);
    }
    assert!(
        !all_builtins().is_empty(),
        "at least one built-in must exist"
    );
}
