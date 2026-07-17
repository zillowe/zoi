use std::fs;
use tempfile::tempdir;
use zoi::pkg::types;
use zoi::utils;

mod common;

#[test]
fn test_install_default_scoping_project() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_current_dir(&root);
    ctx.set_env_var("HOME", &root);

    // Create zoi.lua to trigger project scope
    fs::write(root.join("zoi.lua"), "project({ name = 'test-project' })\n").unwrap();

    let scope = utils::resolve_fallback_scope();
    assert_eq!(scope, types::Scope::Project);
}

#[test]
fn test_install_default_scoping_system_on_zoios() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_sysroot(root.clone());
    ctx.set_env_var("HOME", root.join("home"));
    ctx.set_current_dir(&root); // Ensure we are NOT in a project with zoi.lua

    // Mock ZoiOS
    let os_release_dir = root.join("etc");
    fs::create_dir_all(&os_release_dir).unwrap();
    fs::write(os_release_dir.join("os-release"), "ID=parlex\n").unwrap();

    let scope = utils::resolve_fallback_scope();
    assert_eq!(scope, types::Scope::System);
}

#[test]
fn test_install_default_scoping_user_elsewhere() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_sysroot(root.clone());
    ctx.set_env_var("HOME", root.join("home"));
    ctx.set_current_dir(&root);

    // Mock Generic Linux
    let os_release_dir = root.join("etc");
    fs::create_dir_all(&os_release_dir).unwrap();
    fs::write(os_release_dir.join("os-release"), "ID=ubuntu\n").unwrap();

    let scope = utils::resolve_fallback_scope();
    assert_eq!(scope, types::Scope::User);
}
