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
