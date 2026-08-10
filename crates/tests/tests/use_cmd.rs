//! Integration tests for the `zoi use` command.

use std::fs;

use tempfile::tempdir;
use zoi::cmd::use_cmd;

mod common;

#[test]
fn test_use_cmd_updates_zoi_yaml() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    let zoi_yaml_path = root.join("zoi.yaml");
    fs::write(&zoi_yaml_path, "name: test-project\npkgs: []\n")
        .expect("unwrap failed");

    let pkg_lua_path = root.join("node.pkg.lua");
    fs::write(&pkg_lua_path, r#"metadata({ name = "node", repo = "core", version = "20.0.0", types = {"source"}, maintainer = {name="test", email="test"} })"#).expect("unwrap failed");

    let pkg_spec = pkg_lua_path.to_str().expect("unwrap failed").to_string();

    let _ = use_cmd::run(std::slice::from_ref(&pkg_spec), false);

    let content = fs::read_to_string(&zoi_yaml_path).expect("unwrap failed");
    assert!(content.contains(&pkg_spec));
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

    let config_path = root.join(".zoi/pkgs/config.yaml");
    assert!(config_path.exists(), "Config file should have been created");

    let content = fs::read_to_string(&config_path).expect("unwrap failed");
    assert!(content.contains("python: 3.12.0"));
}
