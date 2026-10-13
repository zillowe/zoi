//! Integration tests for `zoi install --save` / `zoi uninstall --save`, the
//! commands that persist packages into a project's `zoi.lua`.

use std::fs;

use tempfile::tempdir;

mod common;

/// Installing a package with `--save` inside a project must record it in the
/// `packages({...})` block, which is what a later bare `zoi install` replays.
#[test]
fn test_install_save_updates_zoi_lua() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_lua_path = root.join("zoi.lua");
    fs::write(&zoi_lua_path, "project({ name = \"test-project\" })\n")
        .expect("unwrap failed");

    let pkg_lua_path = root.join("node.pkg.lua");
    fs::write(&pkg_lua_path, r#"metadata({ name = "node", repo = "core", version = "20.0.0", description = "test fixture", types = {"source"}, maintainer = {name="test", email="test"} })"#).expect("unwrap failed");

    let pkg_spec = pkg_lua_path.to_str().expect("unwrap failed").to_string();

    zoi::install_sources(
        std::slice::from_ref(&pkg_spec),
        &zoi::SourceInstallOptions {
            yes: true,
            save: true,
            scope_override: Some(zoi::Scope::Project),
            ..Default::default()
        }
    )
    .expect("install should succeed");

    let content = fs::read_to_string(&zoi_lua_path).expect("unwrap failed");
    assert!(
        content.contains(&pkg_spec),
        "expected {pkg_spec} to be recorded in zoi.lua, got:\n{content}"
    );

    // The edited file must still parse as a valid project config.
    let cfg = zoi::project::config::load().expect("edited zoi.lua should load");
    assert_eq!(cfg.pkgs, vec![pkg_spec]);
}

/// `--save` must still record a package that is already installed elsewhere.
/// Installing into a project does not imply the package is new, and the
/// common case is pinning something already present in user scope.
#[test]
fn test_install_save_records_already_installed_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_lua_path = root.join("zoi.lua");
    fs::write(&zoi_lua_path, "project({ name = \"test-project\" })\n")
        .expect("unwrap failed");

    let pkg_lua_path = root.join("tool.pkg.lua");
    fs::write(&pkg_lua_path, r#"metadata({ name = "tool", repo = "core", version = "1.0.0", description = "test fixture", types = {"source"}, maintainer = {name="test", email="test"} })"#).expect("unwrap failed");
    let pkg_spec = pkg_lua_path.to_str().expect("unwrap failed").to_string();

    let options = zoi::SourceInstallOptions {
        yes: true,
        save: true,
        scope_override: Some(zoi::Scope::Project),
        ..Default::default()
    };

    // First install puts it in the project store.
    zoi::install_sources(std::slice::from_ref(&pkg_spec), &options)
        .expect("first install should succeed");

    // Wipe zoi.lua so the package is installed but unrecorded, which is the
    // state this regression is about.
    fs::write(&zoi_lua_path, "project({ name = \"test-project\" })\n")
        .expect("unwrap failed");

    // Second install is a no-op for installation but must still save.
    zoi::install_sources(std::slice::from_ref(&pkg_spec), &options)
        .expect("second install should succeed");

    let content = fs::read_to_string(&zoi_lua_path).expect("unwrap failed");
    assert!(
        content.contains(&pkg_spec),
        "already-installed package must still be recorded, got:\n{content}"
    );
}

/// `--save` outside a project must fail loudly. Silently installing and
/// dropping the user's intent is worse than an error, because the package
/// would look recorded when it is not.
#[test]
fn test_install_save_outside_project_fails() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    assert!(!root.join("zoi.lua").exists(), "precondition: no zoi.lua");

    let err = zoi::install_sources(
        &["@core/eza".to_string()],
        &zoi::SourceInstallOptions {
            yes: true,
            save: true,
            scope_override: Some(zoi::Scope::Project),
            ..Default::default()
        }
    )
    .expect_err("--save outside a project should fail");

    let msg = err.to_string();
    assert!(
        msg.contains("--save requires a 'zoi.lua'"),
        "error should explain the missing zoi.lua, got: {msg}"
    );
}

/// Re-saving a package that is already recorded must not duplicate it, and
/// must leave the rest of the block intact.
#[test]
fn test_add_packages_to_config_does_not_duplicate() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_lua_path = root.join("zoi.lua");
    fs::write(
        &zoi_lua_path,
        "project({ name = \"test-project\" })\npackages({\n    \
         \"@core/eza\",\n})\n"
    )
    .expect("unwrap failed");

    zoi::project::config::add_packages_to_config(&[
        "@community/bat".to_string(),
        "@core/fzf".to_string()
    ])
    .expect("unwrap failed");

    // Re-adding an existing entry must not duplicate it.
    zoi::project::config::add_packages_to_config(&["@core/eza".to_string()])
        .expect("unwrap failed");

    let cfg = zoi::project::config::load().expect("edited zoi.lua should load");
    let mut recorded = cfg.pkgs.clone();
    recorded.sort();
    assert_eq!(
        recorded,
        vec![
            "@community/bat".to_string(),
            "@core/eza".to_string(),
            "@core/fzf".to_string()
        ],
        "each package is recorded exactly once"
    );
}

/// `uninstall --save` must drop the package from `zoi.lua` while leaving the
/// rest of the block intact.
#[test]
fn test_remove_packages_from_zoi_lua() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_lua_path = root.join("zoi.lua");
    fs::write(
        &zoi_lua_path,
        "project({ name = \"test-project\" })\npackages({\n    \
         \"@core/eza\",\n    \"@community/bat\",\n})\n"
    )
    .expect("unwrap failed");

    zoi::project::config::remove_packages_from_config(&[
        "@core/eza".to_string()
    ])
    .expect("unwrap failed");

    let content = fs::read_to_string(&zoi_lua_path).expect("unwrap failed");
    assert!(!content.contains("@core/eza"));
    assert!(content.contains("@community/bat"));

    // The edited file must still load as a valid project config.
    let cfg = zoi::project::config::load().expect("edited zoi.lua should load");
    assert_eq!(cfg.pkgs, vec!["@community/bat".to_string()]);
}
