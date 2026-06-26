use std::fs;
use tempfile::tempdir;
use zoi::pkg::plugin::PluginManager;
use zoi::pkg::shim;

mod common;

#[test]
fn test_shim_resolves_version_from_tool_versions() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);

    fs::write(root.join(".tool-versions"), "node 20.0.0\npython 3.12.0").unwrap();

    let pm = PluginManager::new().unwrap();

    let db_dir = root.join("db");
    ctx.set_env_var("ZOI_DB_DIR", &db_dir);
    let conn = zoi::pkg::db::open_connection("local").unwrap();
    let pkg = zoi::pkg::types::Package {
        name: "node".to_string(),
        repo: "core".to_string(),
        bins: Some(vec!["node".to_string()]),
        ..Default::default()
    };
    zoi::pkg::db::update_package(&conn, &pkg, "local", None, None, None).unwrap();

    let res = shim::resolve_to_installed_bin("node", Some(&pm));

    match res {
        Err(e) => {
            assert!(e.to_string().contains("DB"));
        }
        _ => panic!("Expected error because version is not installed"),
    }
}

#[test]
fn test_tool_versions_traversal() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let sub = root.join("sub/dir");
    fs::create_dir_all(&sub).unwrap();

    fs::write(root.join(".tool-versions"), "node 18.0.0").unwrap();

    ctx.set_current_dir(&sub);

    let pm = PluginManager::new().unwrap();

    let db_dir = root.join("db");
    ctx.set_env_var("ZOI_DB_DIR", &db_dir);
    let conn = zoi::pkg::db::open_connection("local").unwrap();
    let pkg = zoi::pkg::types::Package {
        name: "node".to_string(),
        repo: "core".to_string(),
        bins: Some(vec!["node".to_string()]),
        ..Default::default()
    };
    zoi::pkg::db::update_package(&conn, &pkg, "local", None, None, None).unwrap();

    let res = shim::resolve_to_installed_bin("node", Some(&pm));

    match res {
        Err(e) => {
            assert!(e.to_string().contains("DB"));
        }
        _ => panic!("Expected error"),
    }
}
