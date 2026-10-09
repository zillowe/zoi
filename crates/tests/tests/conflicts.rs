//! Integration tests for package conflict detection and resolution.

use std::fs;

use tempfile::tempdir;
use zoi::pkg::install::preflight::get_conflicts_from_list;
use zoi::pkg::types::{Package, Scope};

mod common;

#[test]
fn test_get_conflicts_from_list_detects_existing_files() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let home = root.join("home");
    fs::create_dir_all(&home).expect("unwrap failed");
    ctx.set_env_var("HOME", home.clone());

    let conflicting_file = home.join("existing_config.txt");
    fs::write(&conflicting_file, "old content").expect("unwrap failed");

    let pkg = Package {
        name: "test-pkg".to_string(),
        scope: Scope::User,
        ..Default::default()
    };

    let file_list = vec![
        "data/usrhome/existing_config.txt".to_string(),
        "data/usrhome/new_file.txt".to_string(),
    ];

    let conflicts = get_conflicts_from_list(file_list, &pkg, None)
        .expect("Should not fail to check conflicts");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts
            .first()
            .expect("Value should exist in test")
            .as_str(),
        conflicting_file.to_string_lossy().to_string()
    );
}

#[test]
fn test_get_conflicts_from_list_ignores_different_scopes() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    common::TestContextGuard::set_sysroot(root.clone());

    let sys_file = root.join("etc/system_config.txt");
    fs::create_dir_all(sys_file.parent().expect("unwrap failed"))
        .expect("unwrap failed");
    fs::write(&sys_file, "system content").expect("unwrap failed");

    let pkg = Package {
        name: "test-pkg".to_string(),
        scope: Scope::User,
        ..Default::default()
    };

    let file_list = vec!["data/usrroot/etc/system_config.txt".to_string()];

    let conflicts = get_conflicts_from_list(file_list, &pkg, None)
        .expect("Should not fail to check conflicts");

    assert_eq!(conflicts.len(), 0);
}

/// `package_files` records paths in placeholder form, such as
/// `${usrroot}/usr/bin/zoi`. `get_other_owners` is queried with real
/// filesystem paths during install, so the two forms have to agree or the
/// ownership check silently never matches and every shared file looks
/// unowned.
///
/// This pins the placeholder form the index is written in, because that is
/// the contract between install, uninstall and this lookup.
#[test]
fn test_placeholder_paths_are_indexed_and_matched_consistently() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    common::TestContextGuard::set_sysroot(root.clone());

    let registry = "conflict-test-registry";
    let conn = zoi_db::open_connection(registry).expect("Failed to open db");

    let owner = Package {
        name: "owner-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        description: "a package that owns a shared file".to_string(),
        ..Default::default()
    };
    let pkg_id = zoi_db::update_package(
        &conn,
        &owner,
        registry,
        Some(Scope::System),
        None,
        Some(&zoi::pkg::types::InstallReason::Direct)
    )
    .expect("Failed to insert package");

    // This is the exact form `pkg_install.rs` pushes into `installed_files`
    // for a file staged into the system root.
    let tracked = "${usrroot}/usr/share/iana-etc/protocols.iana";
    zoi_db::index_package_files(&conn, pkg_id, &[tracked.to_string()])
        .expect("Failed to index files");

    let owners = zoi_db::get_other_owners(&conn, tracked, None)
        .expect("Failed to query owners");
    assert_eq!(owners, vec!["owner-pkg".to_string()]);

    // Excluding the owning package's own id must report no competitor, which
    // is what lets a reinstall rewrite its own files without being treated as
    // a cross-package conflict.
    assert!(
        !zoi_db::has_other_owners(&conn, tracked, pkg_id)
            .expect("Failed to check owners")
    );

    // A path nobody indexed has no owners, so install treats it as a stray
    // file rather than a package overlap.
    let unrelated = "${usrroot}/etc/someone-elses-file";
    assert_eq!(
        zoi_db::get_other_owners(&conn, unrelated, None)
            .expect("Failed to query owners"),
        [] as [std::string::String; 0]
    );
}

/// A second package claiming the same path must be visible as a distinct
/// owner, which is the condition install reports as a conflict between two
/// packages rather than a stray file.
#[test]
fn test_shared_path_reports_the_other_owning_package() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    common::TestContextGuard::set_sysroot(root.clone());

    let registry = "conflict-shared-test-registry";
    let conn = zoi_db::open_connection(registry).expect("Failed to open db");

    let shared = "${usrroot}/etc/protocols";

    let first_pkg = Package {
        name: "first-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        description: "first claim".to_string(),
        ..Default::default()
    };
    let first = zoi_db::update_package(
        &conn,
        &first_pkg,
        registry,
        Some(Scope::System),
        None,
        Some(&zoi::pkg::types::InstallReason::Direct)
    )
    .expect("Failed to insert first package");
    zoi_db::index_package_files(&conn, first, &[shared.to_string()])
        .expect("Failed to index first");

    let second_pkg = Package {
        name: "second-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        description: "second claim".to_string(),
        ..Default::default()
    };
    let second = zoi_db::update_package(
        &conn,
        &second_pkg,
        registry,
        Some(Scope::System),
        None,
        Some(&zoi::pkg::types::InstallReason::Direct)
    )
    .expect("Failed to insert second package");

    // `second-pkg` is installing and does not yet own the path, so the only
    // competitor is `first-pkg`.
    let owners =
        zoi_db::get_other_owners(&conn, shared, Some(second)).expect("query");
    assert_eq!(owners, vec!["first-pkg".to_string()]);

    // Once both own it, uninstall must refuse to remove the file while any
    // other package still claims it.
    zoi_db::index_package_files(&conn, second, &[shared.to_string()])
        .expect("Failed to index second");
    assert!(
        zoi_db::has_other_owners(&conn, shared, first)
            .expect("has_other_owners")
    );
}
