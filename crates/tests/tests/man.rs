//! Integration tests for man page generation and management.

use std::fs;

use tempfile::tempdir;
use zoi::cmd::man;
use zoi::pkg::{config, db, local, types};

mod common;

#[test]
fn test_parse_roff_basic() {
    let roff = r"
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
";
    let md = man::parse_roff(roff,);
    assert!(md.contains("# MYTOOL"));
    assert!(md.contains("## NAME"));
    assert!(md.contains("## SYNOPSIS"));
    assert!(md.contains("**mytool**"));
    assert!(md.contains("## DESCRIPTION"));
}

#[test]
fn test_man_resolution_by_provides() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir",);
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone(),);
    common::TestContextGuard::set_sysroot(root.clone(),);
    ctx.set_env_var("ZOI_DB_DIR", root.join("db",),);

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
        },),
        repos: vec![repo.to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg,).expect("unwrap failed",);

    let conn = db::open_connection(handle,).expect("unwrap failed",);
    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some("1.0.0".to_string(),),
        bins: Some(vec![bin_name.to_string()],),
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
        &[format!("data/pkgstore/bin/{bin_name}"),],
    )
    .expect("unwrap failed",);

    let store_base =
        local::get_store_base_dir(types::Scope::User,).expect("unwrap failed",);
    let pkg_ident =
        zoi::pkg::utils::generate_package_id(handle, repo, pkg_name,);
    let pkg_dir_name =
        zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name,);
    let pkg_path = store_base.join(&pkg_dir_name,);
    let version_dir = pkg_path.join("1.0.0",);
    fs::create_dir_all(&version_dir,).expect("unwrap failed",);
    fs::write(version_dir.join("man.md",), "# Manual Content",)
        .expect("unwrap failed",);

    let latest_path = pkg_path.join("latest",);
    zoi::utils::symlink_file(&version_dir, &latest_path,)
        .expect("unwrap failed",);

    let (resolved_pkg, resolved_handle,) =
        man::resolve_package_for_man(bin_name,).expect("unwrap failed",);
    assert_eq!(resolved_pkg.name, pkg_name);
    assert_eq!(resolved_handle, Some(handle.to_string()));
}

#[test]
fn test_gather_local_manual_pages() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir",);
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone(),);
    common::TestContextGuard::set_sysroot(root.clone(),);

    let pkg_name = "local-man-pkg";
    let handle = "local";
    let repo = "core";
    let version = "1.0.0";

    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some(version.to_string(),),
        ..Default::default()
    };

    let store_base =
        local::get_store_base_dir(types::Scope::User,).expect("unwrap failed",);
    let pkg_ident =
        zoi::pkg::utils::generate_package_id(handle, repo, pkg_name,);
    let pkg_dir_name =
        zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name,);
    let pkg_path = store_base.join(&pkg_dir_name,);
    let version_dir = pkg_path.join(version,);
    let share_man = version_dir.join("share",).join("man",).join("man1",);
    fs::create_dir_all(&share_man,).expect("unwrap failed",);

    fs::write(share_man.join("tool.1",), ".TH TOOL 1\n.SH NAME\ntool",)
        .expect("unwrap failed",);
    fs::write(share_man.join("extra.md",), "# Extra",).expect("unwrap failed",);

    let latest_path = pkg_path.join("latest",);
    zoi::utils::symlink_file(&version_dir, &latest_path,)
        .expect("unwrap failed",);

    let pages = man::gather_manual_pages(&pkg, Some(handle,), false, true,)
        .expect("unwrap failed",);
    assert_eq!(pages.len(), 2);
    assert!(pages.contains_key("tool.1"));
    assert!(pages.contains_key("extra.md"));
    assert!(
        pages
            .get("tool.1")
            .expect("unwrap failed")
            .contains("# TOOL")
    );
    assert!(
        pages
            .get("extra.md")
            .expect("unwrap failed")
            .contains("# Extra")
    );
}

#[test]
fn test_man_run_raw() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir",);
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone(),);
    common::TestContextGuard::set_sysroot(root.clone(),);
    ctx.set_env_var("ZOI_DB_DIR", root.join("db",),);

    let handle = "testreg";
    let repo = "core";
    let pkg_name = "raw-pkg";

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: handle.to_string(),
            url: "http://dummy".to_string(),
            advisory_prefix: None,
            authorities: None,
        },),
        repos: vec![repo.to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg,).expect("unwrap failed",);

    let conn = db::open_connection(handle,).expect("unwrap failed",);
    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some("1.0.0".to_string(),),
        ..Default::default()
    };
    let _pkg_id = db::update_package(
        &conn,
        &pkg,
        handle,
        Some(types::Scope::User,),
        None,
        None,
    )
    .expect("unwrap failed",);

    let store_base =
        local::get_store_base_dir(types::Scope::User,).expect("unwrap failed",);
    let pkg_ident =
        zoi::pkg::utils::generate_package_id(handle, repo, pkg_name,);
    let pkg_dir_name =
        zoi::pkg::utils::get_package_dir_name(&pkg_ident, pkg_name,);
    let pkg_path = store_base.join(&pkg_dir_name,);
    let version_dir = pkg_path.join("1.0.0",);
    fs::create_dir_all(&version_dir,).expect("unwrap failed",);
    fs::write(version_dir.join("man.md",), "# Manual Content",)
        .expect("unwrap failed",);

    let latest_path = pkg_path.join("latest",);
    zoi::utils::symlink_file(&version_dir, &latest_path,)
        .expect("unwrap failed",);

    let res = man::run(pkg_name, false, true, false,);
    assert!(res.is_ok(), "man run raw should succeed: {:?}", res.err());
}
