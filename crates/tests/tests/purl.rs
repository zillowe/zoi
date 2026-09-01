//! Integration tests for Package URL (PURL) resolution.

use std::fs;

use tempfile::tempdir;
use zoi::pkg::purl::resolve_purl;

mod common;

#[test]
fn test_purl_missing_repo() {
    let _ctx = common::TestContextGuard::acquire();
    let result = resolve_purl("pkg:zoi/zoidberg/hello@4.0.0");
    assert!(result.is_err(), "Expected resolve_purl to fail");
    let err = result.expect_err("unwrap_err failed").to_string();
    assert!(
        err.contains("PURL missing repository path"),
        "Expected error message for missing repo. stderr: {err}"
    );
}

#[test]
fn test_purl_unsupported_type() {
    let _ctx = common::TestContextGuard::acquire();
    let result = resolve_purl("pkg:npm/chalk@4.0.0");
    assert!(result.is_err());
    assert!(
        result
            .expect_err("unwrap_err failed")
            .to_string()
            .contains("Unsupported PURL type")
    );
}

#[test]
fn test_fetch_and_store_purl_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");

    let assets_dir = tmp.path().join("assets");
    fs::create_dir_all(assets_dir.join("zillowe/hello"))
        .expect("unwrap failed");
    fs::write(
        assets_dir.join("zillowe/hello/hello.pkg.lua"),
        "metadata({name='hello', version='4.0.0', repo='zillowe', \
         types={'source'}, maintainer={name='test', email='test'}})"
    )
    .expect("unwrap failed");
    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/assets/packages.json"),
        assets_dir.join("packages.json")
    )
    .expect("unwrap failed");

    let db_root = tmp.path().join("db");
    fs::create_dir_all(&db_root).expect("unwrap failed");
    ctx.set_env_var("ZOI_DB_DIR", db_root.to_str().expect("unwrap failed"));

    let result = zoi::pkg::purl::fetch_and_store_purl_package(
        "pkg:zoi/zoidberg/zillowe/hello@4.0.0"
    );
    assert!(
        result.is_ok(),
        "Expected fetch_and_store_purl_package to succeed. err: {:?}",
        result.err()
    );
    let ident = result.expect("unwrap failed");
    assert_eq!(ident, "#zoidberg@zillowe/hello@4.0.0");

    let pkg_path = db_root
        .join("zoidberg")
        .join("zillowe")
        .join("hello")
        .join("hello.pkg.lua");
    assert!(pkg_path.exists(), "pkg.lua should be stored in DB");
}
