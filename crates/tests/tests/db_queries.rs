//! Regression tests for scope/registry-aware database helpers.

use tempfile::tempdir;
use zoi::pkg::{db, types};

mod common;

fn pkg(name: &str, bins: Option<Vec<&str>>) -> types::Package {
    types::Package {
        name: name.to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        package_type: types::PackageType::Package,
        bins: bins.map(|b| b.into_iter().map(String::from).collect()),
        ..Default::default()
    }
}

#[test]
fn test_get_package_id_scoped_distinguishes_scopes() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());
    let conn = db::open_connection("local").expect("unwrap failed");

    let package = pkg("scoped-id", None);
    let user_id = db::update_package(
        &conn,
        &package,
        "local",
        Some(types::Scope::User),
        None,
        None
    )
    .expect("unwrap failed");
    let system_id = db::update_package(
        &conn,
        &package,
        "local",
        Some(types::Scope::System),
        None,
        None
    )
    .expect("unwrap failed");
    assert_ne!(user_id, system_id);

    assert_eq!(
        db::get_package_id_scoped(
            &conn,
            "scoped-id",
            None,
            "core",
            "local",
            Some(types::Scope::User)
        )
        .expect("unwrap failed"),
        user_id
    );
    assert_eq!(
        db::get_package_id_scoped(
            &conn,
            "scoped-id",
            None,
            "core",
            "local",
            Some(types::Scope::System)
        )
        .expect("unwrap failed"),
        system_id
    );
    assert!(
        db::get_package_id_scoped(
            &conn,
            "scoped-id",
            None,
            "core",
            "local",
            Some(types::Scope::Project)
        )
        .is_err()
    );
}

#[test]
fn test_delete_package_only_removes_matching_registry() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());
    let conn = db::open_connection("local").expect("unwrap failed");

    let package = pkg("multi-reg", None);
    for registry in ["reg-a", "reg-b"] {
        db::update_package(
            &conn,
            &package,
            registry,
            Some(types::Scope::User),
            None,
            None
        )
        .expect("unwrap failed");
    }

    db::delete_package(
        &conn,
        "multi-reg",
        None,
        "core",
        Some(types::Scope::User),
        Some("reg-a")
    )
    .expect("unwrap failed");

    let remaining: Vec<String> = conn
        .prepare("SELECT registry FROM packages WHERE name = 'multi-reg'")
        .expect("unwrap failed")
        .query_map([], |row| row.get(0))
        .expect("unwrap failed")
        .map(|r| r.expect("unwrap failed"))
        .collect();
    assert_eq!(remaining, vec!["reg-b".to_string()]);
}

#[test]
fn test_find_exact_bin_providers_is_exact_and_scoped() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());
    let conn = db::open_connection("local").expect("unwrap failed");

    let user = Some(types::Scope::User);
    db::update_package(&conn, &pkg("git", None), "local", user, None, None)
        .expect("unwrap failed");
    db::update_package(
        &conn,
        &pkg("git-lfs", Some(vec!["git-lfs"])),
        "local",
        user,
        None,
        None
    )
    .expect("unwrap failed");
    db::update_package(
        &conn,
        &pkg("other-git", Some(vec!["git"])),
        "local",
        Some(types::Scope::System),
        None,
        None
    )
    .expect("unwrap failed");

    let providers = db::find_exact_bin_providers(&conn, "git", user)
        .expect("unwrap failed");
    let names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
    // `git` provides its implicit binary; `git-lfs` only contains the term
    // and `other-git` lives in another scope.
    assert_eq!(names, vec!["git"]);

    let providers = db::find_exact_bin_providers(&conn, "git-lfs", user)
        .expect("unwrap failed");
    assert_eq!(providers.len(), 1);
    let provider = providers.first().expect("one provider");
    assert_eq!(provider.name, "git-lfs");
    assert_eq!(provider.registry.as_deref(), Some("local"));

    let providers = db::find_exact_bin_providers(&conn, "lfs", user)
        .expect("unwrap failed");
    assert!(providers.is_empty(), "substring matches must not count");
}

#[test]
fn test_get_dependents_ignores_non_dependents_without_error() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    ctx.set_env_var("ZOI_DB_DIR", tmp.path());
    let conn = db::open_connection("local").expect("unwrap failed");

    db::update_package(
        &conn,
        &pkg("standalone", None),
        "local",
        Some(types::Scope::User),
        None,
        None
    )
    .expect("unwrap failed");
    drop(conn);

    let dependents = db::get_dependents("local", "target-pkg", None)
        .expect("rows that do not depend on the target are not errors");
    assert!(dependents.is_empty());
}
