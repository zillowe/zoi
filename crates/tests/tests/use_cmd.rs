//! Integration tests for the `zoi use` command.

use std::fs;

use tempfile::tempdir;
use zoi::cmd::use_cmd;

mod common;

#[test]
fn test_use_cmd_updates_zoi_lua() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_lua_path = root.join("zoi.lua");
    fs::write(&zoi_lua_path, "project({ name = \"test-project\" })\n")
        .expect("unwrap failed");

    let pkg_lua_path = root.join("node.pkg.lua");
    fs::write(&pkg_lua_path, r#"metadata({ name = "node", repo = "core", version = "20.0.0", types = {"source"}, maintainer = {name="test", email="test"} })"#).expect("unwrap failed");

    let pkg_spec = pkg_lua_path.to_str().expect("unwrap failed").to_string();

    let _ = use_cmd::run(std::slice::from_ref(&pkg_spec), false);

    let content = fs::read_to_string(&zoi_lua_path).expect("unwrap failed");
    assert!(content.contains(&pkg_spec));
}

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

#[test]
fn test_use_cmd_global_updates_config() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);

    let pkg_lua_path = root.join("python.pkg.lua");
    fs::write(&pkg_lua_path, r#"metadata({ name = "python", repo = "core", version = "3.12.0", types = {"source"}, maintainer = {name="test", email="test"} })"#).expect("unwrap failed");

    let pkg_spec =
        format!("{}@3.12.0", pkg_lua_path.to_str().expect("unwrap failed"));

    let _ = use_cmd::run(&[pkg_spec], true);

    let config_path = zoi::pkg::utils::get_user_config_dir()
        .expect("unwrap failed")
        .join("config.yaml");
    assert!(config_path.exists(), "Config file should have been created");

    let content = fs::read_to_string(&config_path).expect("unwrap failed");
    assert!(content.contains("python: 3.12.0"));
}
