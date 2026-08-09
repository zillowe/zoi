//! Integration tests for package "provides" functionality and virtual packages.

use tempfile::tempdir;
use zoi::pkg::{db, types};

mod common;

#[test]
fn test_find_provides_logic() {
    let mut ctx = common::TestContextGuard::acquire();
    let dir = tempdir().expect("unwrap failed",);
    let db_dir = dir.path().to_path_buf();
    ctx.set_env_var("ZOI_DB_DIR", &db_dir,);

    let handle = "local";
    let conn = db::open_connection(handle,).expect("unwrap failed",);

    let pkg = types::Package {
        name: "git".to_string(),
        repo: "core".to_string(),
        version: Some("2.40.0".to_string(),),
        bins: Some(vec!["git".to_string()],),
        package_type: types::PackageType::Package,
        ..Default::default()
    };

    let pkg_id = db::update_package(
        &conn,
        &pkg,
        handle,
        Some(types::Scope::User,),
        None,
        None,
    )
    .expect("unwrap failed",);
    db::index_package_files(
        &conn,
        pkg_id,
        &["data/pkgstore/bin/git".to_string(),],
    )
    .expect("unwrap failed",);

    let results = db::find_provides(handle, "git",).expect("unwrap failed",);
    assert!(!results.is_empty());
    assert_eq!(
        results.first().expect("Value should exist in test").0.name,
        "git"
    );
}
