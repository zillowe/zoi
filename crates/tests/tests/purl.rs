use std::fs;
use tempfile::tempdir;
use zoi::pkg::purl::resolve_purl;

mod common;

fn setup_purl_test() -> (common::TestContextGuard, tempfile::TempDir) {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().unwrap();
    let reg_json = tmp.path().join("registries.json");
    let assets_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets");

    let registries = format!(
        r#"{{
      "version": "1",
      "zoidberg": {{
        "name": "Zoidberg",
        "description": "Test registry",
        "git": "{}",
        "branch": "main"
      }}
    }}"#,
        assets_path.to_string_lossy().replace("\\", "/")
    );
    fs::write(&reg_json, registries).unwrap();
    ctx.set_env_var("ZOI_PURL_DB_URL", reg_json.to_str().unwrap());
    (ctx, tmp)
}

#[test]
fn test_purl_missing_repo() {
    let _ctx = common::TestContextGuard::acquire();
    let result = resolve_purl("pkg:zoi/zoidberg/hello@4.0.0");
    assert!(result.is_err(), "Expected resolve_purl to fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("PURL missing repository path"),
        "Expected error message for missing repo. stderr: {}",
        err
    );
}

#[test]
fn test_purl_repo_mismatch() {
    let (_ctx, _tmp) = setup_purl_test();

    let result = resolve_purl("pkg:zoi/zoidberg/asadasd/hello@4.0.0");
    assert!(result.is_err(), "Expected resolve_purl to fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Repository mismatch in PURL") || err.contains("not found in registry"),
        "Expected error message for repo mismatch. err: {}",
        err
    );
}

#[test]
fn test_purl_unsupported_type() {
    let _ctx = common::TestContextGuard::acquire();
    let result = resolve_purl("pkg:npm/chalk@4.0.0");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Unsupported PURL type")
    );
}

#[test]
fn test_purl_successful_resolution() {
    let (_ctx, _tmp) = setup_purl_test();

    let result = resolve_purl("pkg:zoi/zoidberg/zillowe/hello@4.0.0");
    assert!(
        result.is_ok(),
        "Expected resolve_purl to succeed. err: {:?}",
        result.err()
    );
    let resolved = result.unwrap();
    assert_eq!(resolved.registry_handle, "zoidberg");
    assert_eq!(resolved.package_path, "hello");
    assert_eq!(resolved.package_info.repo, "zillowe");
}

#[test]
fn test_fetch_and_store_purl_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().unwrap();

    let assets_dir = tmp.path().join("assets");
    fs::create_dir_all(assets_dir.join("zillowe/hello")).unwrap();
    fs::write(assets_dir.join("zillowe/hello/hello.pkg.lua"), "metadata({name='hello', version='4.0.0', repo='zillowe', types={'source'}, maintainer={name='test', email='test'}})").unwrap();
    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/packages.json"),
        assets_dir.join("packages.json"),
    )
    .unwrap();

    let reg_json = tmp.path().join("registries.json");
    let registries = format!(
        r#"{{
      "version": "1",
      "zoidberg": {{
        "name": "Zoidberg",
        "description": "Test registry",
        "git": "{}",
        "branch": "main"
      }}
    }}"#,
        assets_dir.to_string_lossy().replace("\\", "/")
    );
    fs::write(&reg_json, registries).unwrap();
    ctx.set_env_var("ZOI_PURL_DB_URL", reg_json.to_str().unwrap());

    let db_root = tmp.path().join("db");
    fs::create_dir_all(&db_root).unwrap();
    ctx.set_env_var("ZOI_DB_DIR", db_root.to_str().unwrap());

    let result =
        zoi::pkg::purl::fetch_and_store_purl_package("pkg:zoi/zoidberg/zillowe/hello@4.0.0");
    assert!(
        result.is_ok(),
        "Expected fetch_and_store_purl_package to succeed. err: {:?}",
        result.err()
    );
    let ident = result.unwrap();
    assert_eq!(ident, "#zoidberg@zillowe/hello@4.0.0");

    let pkg_path = db_root
        .join("zoidberg")
        .join("zillowe")
        .join("hello")
        .join("hello.pkg.lua");
    assert!(pkg_path.exists(), "pkg.lua should be stored in DB");
}
