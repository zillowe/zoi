use std::fs;
use tempfile::tempdir;
use zoi::pkg::install::util::get_conflicts_from_list;
use zoi::pkg::types::{Package, Scope};

mod common;

#[test]
fn test_get_conflicts_from_list_detects_existing_files() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    ctx.set_env_var("HOME", home.clone());

    let conflicting_file = home.join("existing_config.txt");
    fs::write(&conflicting_file, "old content").unwrap();

    let pkg = Package {
        name: "test-pkg".to_string(),
        scope: Scope::User,
        ..Default::default()
    };

    let file_list = vec![
        "data/usrhome/existing_config.txt".to_string(),
        "data/usrhome/new_file.txt".to_string(),
    ];

    let conflicts =
        get_conflicts_from_list(file_list, &pkg, None).expect("Should not fail to check conflicts");

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0], conflicting_file.to_string_lossy().to_string());
}

#[test]
fn test_get_conflicts_from_list_ignores_different_scopes() {
    let ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_sysroot(root.clone());

    let sys_file = root.join("etc/system_config.txt");
    fs::create_dir_all(sys_file.parent().unwrap()).unwrap();
    fs::write(&sys_file, "system content").unwrap();

    let pkg = Package {
        name: "test-pkg".to_string(),
        scope: Scope::User,
        ..Default::default()
    };

    let file_list = vec!["data/usrroot/etc/system_config.txt".to_string()];

    let conflicts =
        get_conflicts_from_list(file_list, &pkg, None).expect("Should not fail to check conflicts");

    assert_eq!(conflicts.len(), 0);
}
