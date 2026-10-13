//! Integration tests for `zoi pin` / `zoi unpin`.
//!
//! Pinning writes two stores at once - the upgrade guard in `pinned.json` and
//! the shim preference in the `versions:` map of `config.yaml` - so these tests
//! cover both halves and the unpin that has to undo them.

use std::collections::HashMap;
use std::fs;

use tempfile::tempdir;
use zoi::pkg::pin;

mod common;

fn write_pkg(root: &std::path::Path, bins: &str) -> String {
    let path = root.join("tool.pkg.lua");
    fs::write(
        &path,
        format!(
            r#"metadata({{ name = "tool", repo = "core", version = "1.0.0", description = "test fixture", types = {{"source"}}, bins = {bins}, maintainer = {{name="test", email="test"}} }})"#
        )
    )
    .expect("unwrap failed");
    path.to_str().expect("unwrap failed").to_string()
}

fn user_config() -> std::path::PathBuf {
    zoi::pkg::utils::get_user_config_dir()
        .expect("unwrap failed")
        .join("config.yaml")
}

/// Pinning must record the preferred global version for every binary the
/// package provides, so the shim resolves to the pinned version rather than
/// whatever happens to be latest.
#[test]
fn test_pin_records_global_versions_for_bins() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_spec = write_pkg(&root, r#"{"tool", "toolctl"}"#);

    zoi::cmd::pin::run(&pkg_spec, "1.0.0", true).expect("pin should succeed");

    let content = fs::read_to_string(user_config()).expect("unwrap failed");
    assert!(
        content.contains("tool: 1.0.0"),
        "expected a versions entry for 'tool', got:\n{content}"
    );
    assert!(
        content.contains("toolctl: 1.0.0"),
        "expected a versions entry for 'toolctl', got:\n{content}"
    );
}

/// A package that declares no `bins` still gets an entry keyed by its own
/// name, which covers the common case where binary and package share a name.
#[test]
fn test_pin_without_bins_falls_back_to_package_name() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_spec = write_pkg(&root, "nil");

    zoi::cmd::pin::run(&pkg_spec, "1.0.0", true).expect("pin should succeed");

    let content = fs::read_to_string(user_config()).expect("unwrap failed");
    assert!(
        content.contains("tool: 1.0.0"),
        "expected a versions entry for 'tool', got:\n{content}"
    );
}

/// Unpinning must clear both stores. Leaving the `versions:` entry behind
/// would keep pointing the shim at the unpinned version forever.
#[test]
fn test_unpin_clears_both_stores() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_spec = write_pkg(&root, r#"{"tool", "toolctl"}"#);

    zoi::cmd::pin::run(&pkg_spec, "1.0.0", true).expect("pin should succeed");
    assert!(
        pin::is_pinned("tool").expect("unwrap failed"),
        "precondition: package should be pinned"
    );

    zoi::cmd::unpin::run(&pkg_spec, true).expect("unpin should succeed");

    assert!(
        !pin::is_pinned("tool").expect("unwrap failed"),
        "pinned.json entry should be gone"
    );

    let cfg = zoi::pkg::config::read_user_config().expect("unwrap failed");
    assert!(
        !cfg.versions.contains_key("tool")
            && !cfg.versions.contains_key("toolctl"),
        "versions entries should be gone, got: {:?}",
        cfg.versions
    );
}

/// Unpinning one package must not disturb another package's preference for a
/// binary name they happen to share.
#[test]
fn test_unpin_preserves_other_packages_versions() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    let pkg_spec = write_pkg(&root, r#"{"tool"}"#);

    // A second package's preference, as if a different package also shipped a
    // binary called "shared".
    let mut shared_versions: std::collections::HashMap<String, String> =
        HashMap::new();
    shared_versions.insert("shared".to_string(), "2.0.0".to_string());
    zoi::pkg::config::update_global_versions(shared_versions)
        .expect("unwrap failed");

    zoi::cmd::pin::run(&pkg_spec, "1.0.0", true).expect("pin should succeed");
    zoi::cmd::unpin::run(&pkg_spec, true).expect("unpin should succeed");

    let cfg = zoi::pkg::config::read_user_config().expect("unwrap failed");
    assert_eq!(
        cfg.versions.get("shared"),
        Some(&"2.0.0".to_string()),
        "another package's preference must survive, got: {:?}",
        cfg.versions
    );
}
