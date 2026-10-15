//! Integration tests for the `init` command and project-scope defaults.

use std::fs;

use tempfile::tempdir;

mod common;

fn read(root: &std::path::Path) -> String {
    fs::read_to_string(root.join("zoi.lua")).expect("read zoi.lua")
}

// `zoi init` writes a zoi.lua that is immediately usable: it must parse, and
// it must already contain an empty `packages({})` block for `--save` to append
// to.
#[test]
fn test_init_writes_usable_project() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    zoi::cmd::init::run(true, false).expect("init should succeed");

    let out = read(&root);
    assert!(out.contains("project("), "should declare a project:\n{out}");
    assert!(
        out.contains("packages({"),
        "should pre-create an empty packages block:\n{out}"
    );

    let cfg =
        zoi::project::config::load().expect("generated zoi.lua should load");
    assert!(cfg.pkgs.is_empty(), "new project should have no packages");
    assert!(!cfg.name.is_empty(), "project should get a non-empty name");
}

// Overwriting someone's existing project config by accident would lose work,
// so it has to be explicit.
#[test]
fn test_init_refuses_to_overwrite() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    fs::write(root.join("zoi.lua"), "project({ name = \"keep-me\" })\n")
        .expect("write zoi.lua");

    let err = zoi::cmd::init::run(true, false)
        .expect_err("init should refuse to overwrite");

    assert!(
        err.to_string().contains("already exists"),
        "error should mention the existing file, got: {err}"
    );
    assert!(
        read(&root).contains("keep-me"),
        "existing zoi.lua must be untouched"
    );
}

// `--force` is the documented escape hatch.
#[test]
fn test_init_force_overwrites() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    fs::write(root.join("zoi.lua"), "project({ name = \"old\" })\n")
        .expect("write zoi.lua");

    zoi::cmd::init::run(true, true).expect("init --force should succeed");
    assert!(!read(&root).contains("old"));
}

// The generated file is a Lua script, so a directory name containing quotes or
// newlines must not be able to break out of the string literal.
#[test]
fn test_init_sanitizes_directory_name() {
    let mut ctx = common::TestContextGuard::acquire();
    // A directory name that would be a syntax error, or an injection, if it
    // were written into the Lua string verbatim.
    let tmp = tempdir().expect("temp dir");
    let nasty = tmp.path().join("bad\"name\"");
    fs::create_dir_all(&nasty).expect("create dir");
    ctx.set_current_dir(&nasty);

    zoi::cmd::init::run(true, false).expect("init should succeed");

    let out = read(&nasty);
    // Parsing is the real assertion: a broken literal would fail to load.
    let cfg =
        zoi::project::config::load().expect("sanitized zoi.lua should load");
    assert!(!cfg.name.is_empty(), "name should be non-empty");
    assert!(
        !out.contains("bad\"name\""),
        "quote must be neutralised:\n{out}"
    );
}

// The name must not start with a character that reads as a flag or a path.
#[test]
fn test_init_name_has_no_leading_punctuation() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let odd = tmp.path().join("--weird--");
    fs::create_dir_all(&odd).expect("create dir");
    ctx.set_current_dir(&odd);

    zoi::cmd::init::run(true, false).expect("init should succeed");

    let cfg = zoi::project::config::load().expect("zoi.lua should load");
    assert!(
        !cfg.name.starts_with('-') && !cfg.name.starts_with('.'),
        "name should not start with '-': got {:?}",
        cfg.name
    );
}

/// `uninstall` now resolves its default scope with the same helper `install`
/// uses, so a directory containing a `zoi.lua` defaults to the project scope
/// instead of being forced to `User`.
#[test]
fn test_fallback_scope_is_project_inside_a_project() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);

    // Without a zoi.lua the fallback is User (or System on ZoiOS).
    let before = zoi::pkg::utils::resolve_fallback_scope();
    assert_ne!(before, zoi::Scope::Project);

    fs::write(root.join("zoi.lua"), "project({ name = \"p\" })\n")
        .expect("write zoi.lua");

    assert_eq!(
        zoi::pkg::utils::resolve_fallback_scope(),
        zoi::Scope::Project
    );
}
