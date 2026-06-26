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
        Some(&types::InstallReason::Direct),
    )
    .expect("Should insert pkg from reg-a");

    let id_b = db::update_package(
        &conn,
        &pkg,
        handle_b,
        Some(types::Scope::User),
        None,
        Some(&types::InstallReason::Direct),
    )
    .expect("Should insert pkg from reg-b without colliding with reg-a");

    assert_ne!(
        id_a, id_b,
        "Packages from different registries should have different IDs and not collide"
    );

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM packages WHERE name = 'shared-pkg'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 2, "Both packages should exist in the database");
}
