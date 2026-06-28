use std::fs;
use tempfile::tempdir;
use zoi::cmd::man;
use zoi::pkg::{config, db, local, types};

mod common;

#[test]
fn test_manspec_deserialization() {
    let single_json = r#""https://example.com/man.md""#;
    let single: types::ManSpec = serde_json::from_str(single_json).unwrap();
    match single {
        types::ManSpec::Single(s) => assert_eq!(s, "https://example.com/man.md"),
        _ => panic!("Expected Single variant"),
    }

    let multiple_json = r#"["url1", "url2"]"#;
    let multiple: types::ManSpec = serde_json::from_str(multiple_json).unwrap();
    match multiple {
        types::ManSpec::Multiple(m) => assert_eq!(m, vec!["url1", "url2"]),
        _ => panic!("Expected Multiple variant"),
    }

    let map_json = r#"{"page1": "url1", "page2": "url2"}"#;
    let map: types::ManSpec = serde_json::from_str(map_json).unwrap();
    match map {
        types::ManSpec::Map(m) => {
            assert_eq!(m.get("page1").unwrap(), "url1");
            assert_eq!(m.get("page2").unwrap(), "url2");
        }
        _ => panic!("Expected Map variant"),
    }
}

#[test]
fn test_parse_roff_basic() {
    let roff = r#"
.TH MYTOOL 1
.SH NAME
mytool \- a test tool
.SH SYNOPSIS
.B mytool
[\fIOPTIONS\fR]
.SH DESCRIPTION
.PP
This is a test tool.
.B \-\-help
show help.
"#;
    let md = man::parse_roff(roff);
    assert!(md.contains("# MYTOOL"));
    assert!(md.contains("## NAME"));
    assert!(md.contains("## SYNOPSIS"));
    assert!(md.contains("**mytool**"));
    assert!(md.contains("## DESCRIPTION"));
}

#[test]
fn test_man_resolution_by_provides() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));

    let handle = "testreg";
    let repo = "core";
    let pkg_name = "test-pkg";
    let bin_name = "test-cmd";

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: handle.to_string(),
            url: "http://dummy".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        repos: vec![repo.to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let conn = db::open_connection(handle).unwrap();
    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some("1.0.0".to_string()),
        bins: Some(vec![bin_name.to_string()]),
        ..Default::default()
    };
    let pkg_id =
        db::update_package(&conn, &pkg, handle, Some(types::Scope::User), None, None).unwrap();
    db::index_package_files(&conn, pkg_id, &[format!("data/pkgstore/bin/{}", bin_name)]).unwrap();

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_ident = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join("1.0.0");
    fs::create_dir_all(&version_dir).unwrap();
    fs::write(version_dir.join("man.md"), "# Manual Content").unwrap();

    let latest_path = pkg_path.join("latest");
    zoi::utils::symlink_file(&version_dir, &latest_path).unwrap();

    let (resolved_pkg, resolved_handle) = man::resolve_package_for_man(bin_name).unwrap();
    assert_eq!(resolved_pkg.name, pkg_name);
    assert_eq!(resolved_handle, Some(handle.to_string()));
}

#[test]
fn test_gather_local_manual_pages() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pkg_name = "local-man-pkg";
    let handle = "local";
    let repo = "core";
    let version = "1.0.0";

    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some(version.to_string()),
        ..Default::default()
    };

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_ident = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let share_man = version_dir.join("share").join("man").join("man1");
    fs::create_dir_all(&share_man).unwrap();

    fs::write(share_man.join("tool.1"), ".TH TOOL 1\n.SH NAME\ntool").unwrap();
    fs::write(share_man.join("extra.md"), "# Extra").unwrap();

    let latest_path = pkg_path.join("latest");
    zoi::utils::symlink_file(&version_dir, &latest_path).unwrap();

    let pages = man::gather_manual_pages(&pkg, &Some(handle.to_string()), false, true).unwrap();
    assert_eq!(pages.len(), 2);
    assert!(pages.contains_key("tool.1"));
    assert!(pages.contains_key("extra.md"));
    assert!(pages.get("tool.1").unwrap().contains("# TOOL"));
    assert!(pages.get("extra.md").unwrap().contains("# Extra"));
}

#[test]
fn test_man_run_raw() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));

    let handle = "testreg";
    let repo = "core";
    let pkg_name = "raw-pkg";

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: handle.to_string(),
            url: "http://dummy".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        repos: vec![repo.to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let conn = db::open_connection(handle).unwrap();
    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some("1.0.0".to_string()),
        ..Default::default()
    };
    let _pkg_id =
        db::update_package(&conn, &pkg, handle, Some(types::Scope::User), None, None).unwrap();

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_ident = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join("1.0.0");
    fs::create_dir_all(&version_dir).unwrap();
    fs::write(version_dir.join("man.md"), "# Manual Content").unwrap();

    let latest_path = pkg_path.join("latest");
    zoi::utils::symlink_file(&version_dir, &latest_path).unwrap();

    let res = man::run(pkg_name, false, true, false);
    assert!(res.is_ok(), "man run raw should succeed: {:?}", res.err());
}
