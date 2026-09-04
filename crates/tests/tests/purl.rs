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
fn test_registry_index_parses_list_form_build_dependencies() {
    // Published registry indexes express per-type build dependencies as a
    // list of `{type, packages}` entries. This shape must deserialize into
    // `BuildDependencies::List`.
    let index: zoi::pkg::purl::RegistryIndex = serde_json::from_str(
        r#"{
            "version": "2",
            "packages": {
                "@extra/bacon": {
                    "repo": "extra",
                    "repo_type": "official",
                    "version": "3.24.0",
                    "description": "A background rust code checker",
                    "sub_packages": [],
                    "main_sub_packages": [],
                    "vuln": [],
                    "dependencies": {
                        "runtime": ["native:cargo: for use with Rust"],
                        "build": [
                            {
                                "type": "source",
                                "packages": ["pacman:rust", "pacman:git"]
                            }
                        ],
                        "test": []
                    }
                }
            }
        }"#
    )
    .expect("registry index with list-form build deps should parse");

    let entry = index
        .packages
        .get("@extra/bacon")
        .expect("entry should exist");
    let deps = entry.dependencies.as_ref().expect("deps should exist");
    match deps.build.as_ref().expect("build deps should exist") {
        zoi::pkg::types::BuildDependencies::List(entries) => {
            assert_eq!(entries.len(), 1);
            let entry =
                entries.first().expect("build entries should not be empty");
            assert_eq!(entry.build_type, "source");
            assert_eq!(entry.packages, vec!["pacman:rust", "pacman:git"]);
        }
        other => panic!("expected List build deps, got {other:?}")
    }
}

#[test]
fn test_fetch_and_store_purl_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");

    let assets_dir = tmp.path().join("assets");
    fs::create_dir_all(assets_dir.join("zillowe/hello"))
        .expect("unwrap failed");
    let fixture_lua = "metadata({name='hello', version='4.0.0', \
                       repo='zillowe', types={'source'}, \
                       maintainer={name='test', email='test'}})";
    fs::write(assets_dir.join("zillowe/hello/hello.pkg.lua"), fixture_lua)
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

    // Resolve against the local fixture directory instead of the network so
    // this test is hermetic: `git` as a filesystem path makes the fetch
    // functions read straight from disk.
    let mut central_db = std::collections::HashMap::new();
    central_db.insert(
        "zoidberg".to_string(),
        zoi::pkg::purl::RegistryInfo {
            name: "Zoidberg".to_string(),
            description: "test fixture registry".to_string(),
            git: assets_dir.to_str().expect("unwrap failed").to_string(),
            branch: "main".to_string()
        }
    );

    let result = zoi::pkg::purl::fetch_and_store_purl_package_with_db(
        "pkg:zoi/zoidberg/zillowe/hello@4.0.0",
        &central_db
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
    // The stored definition must be byte-identical to the local fixture,
    // proving it came from disk rather than the network.
    let stored = fs::read_to_string(&pkg_path).expect("unwrap failed");
    assert_eq!(stored, fixture_lua);
}
