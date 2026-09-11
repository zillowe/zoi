//! Integration tests for database collision handling.

use tempfile::tempdir;
use zoi::pkg::{db, types};
mod common;

#[test]
fn test_db_unique_constraint_includes_registry() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());

    let handle_a = "reg-a";
    let handle_b = "reg-b";

    let conn = db::open_connection("local").expect("Failed to open local db");

    let pkg = types::Package {
        name: "shared-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        package_type: types::PackageType::Package,
        ..Default::default()
    };

    let id_a = db::update_package(
        &conn,
        &pkg,
        handle_a,
        Some(types::Scope::User),
        None,
        Some(&types::InstallReason::Direct)
    )
    .expect("Should insert pkg from reg-a");

    let id_b = db::update_package(
        &conn,
        &pkg,
        handle_b,
        Some(types::Scope::User),
        None,
        Some(&types::InstallReason::Direct)
    )
    .expect("Should insert pkg from reg-b without colliding with reg-a");

    assert_ne!(
        id_a, id_b,
        "Packages from different registries should have different IDs and not \
         collide"
    );

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM packages WHERE name = 'shared-pkg'",
            [],
            |row| row.get(0)
        )
        .expect("unwrap failed");

    assert_eq!(count, 2, "Both packages should exist in the database");
}

#[test]
fn test_db_upsert_with_null_sub_package_does_not_duplicate() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());

    let conn = db::open_connection("local").expect("Failed to open local db");

    let mut pkg = types::Package {
        name: "dup-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        package_type: types::PackageType::Package,
        ..Default::default()
    };

    let id_first = db::update_package(
        &conn,
        &pkg,
        "local",
        Some(types::Scope::User),
        None,
        Some(&types::InstallReason::Direct)
    )
    .expect("Should insert pkg");

    // Simulate a reinstall/upgrade: same identity, new version, NULL
    // sub_package. SQLite UNIQUE treats NULLs as distinct, so a naive ON
    // CONFLICT upsert would pile up a second identical row.
    pkg.version = Some("1.0.1".to_string());
    let id_second = db::update_package(
        &conn,
        &pkg,
        "local",
        Some(types::Scope::User),
        None,
        Some(&types::InstallReason::Direct)
    )
    .expect("Should update pkg in place");

    assert_eq!(
        id_first, id_second,
        "Reinstall should update the existing row, not insert a duplicate"
    );

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM packages WHERE name = 'dup-pkg'",
            [],
            |row| row.get(0)
        )
        .expect("unwrap failed");
    assert_eq!(count, 1, "Only one row should exist after reinstall");

    let version: String = conn
        .query_row(
            "SELECT version FROM packages WHERE name = 'dup-pkg'",
            [],
            |row| row.get(0)
        )
        .expect("unwrap failed");
    assert_eq!(version, "1.0.1", "Version should be updated in place");
}

#[test]
fn test_db_delete_package_is_scope_specific() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());

    let conn = db::open_connection("local").expect("Failed to open local db");

    let pkg = types::Package {
        name: "scoped-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        package_type: types::PackageType::Package,
        ..Default::default()
    };

    for scope in [types::Scope::User, types::Scope::System] {
        db::update_package(
            &conn,
            &pkg,
            "local",
            Some(scope),
            None,
            Some(&types::InstallReason::Direct)
        )
        .expect("Should insert pkg");
    }

    db::delete_package(
        &conn,
        "scoped-pkg",
        None,
        "core",
        Some(types::Scope::System)
    )
    .expect("Should delete system row");

    let remaining: Vec<String> = conn
        .prepare("SELECT scope FROM packages WHERE name = 'scoped-pkg'")
        .expect("unwrap failed")
        .query_map([], |row| row.get(0))
        .expect("unwrap failed")
        .map(|r| r.expect("unwrap failed"))
        .collect();
    assert_eq!(
        remaining,
        vec!["user".to_string()],
        "Uninstalling one scope must not remove the other"
    );
}
