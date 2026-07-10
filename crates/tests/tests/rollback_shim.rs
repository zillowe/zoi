use std::fs;
use tempfile::tempdir;
use zoi::pkg::{local, shim, transaction, types};
mod common;

#[test]
fn test_rollback_restores_shims() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    ctx.set_env_var("HOME", &home);
    ctx.set_current_dir(&root);

    let pkg_name = "shim-restore-test";
    let version = "1.0.0";
    let handle = "local";
    let repo = "core";

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_id = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let bin_path = bin_dir.join("test-cmd");
    fs::write(&bin_path, "echo hello").unwrap();

    let manifest = types::InstallManifest {
        name: pkg_name.to_string(),
        version: version.to_string(),
        revision: "1".to_string(),
        sub_package: None,
        repo: repo.to_string(),
        repo_type: "official".to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        description: "".to_string(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: Some(vec!["test-cmd".to_string()]),
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        dependencies_v2: None,
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        platform: zoi_core::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: vec![bin_path.to_string_lossy().to_string()],
        installed_size: None,
        sandbox: None,
        completions: None,
    };

    local::write_manifest(&manifest).unwrap();

    let bin_root = if cfg!(windows) {
        root.join("ProgramData/zoi/pkgs/bin")
    } else {
        root.join("home/.zoi/pkgs/bin")
    };
    fs::create_dir_all(&bin_root).unwrap();
    let shim_path = bin_root.join("test-cmd");
    shim::create_shim(&shim_path).unwrap();
    assert!(shim_path.exists());

    let mut trans = transaction::begin().unwrap();
    let id = trans.id.clone();
    transaction::record_operation(
        &mut trans,
        types::TransactionOperation::Uninstall {
            manifest: Box::new(manifest.clone()),
        },
    )
    .unwrap();

    fs::remove_file(&shim_path).unwrap();
    assert!(!shim_path.exists());

    transaction::rollback(&id).unwrap();

    let installed = local::is_package_installed(pkg_name, None, types::Scope::User).unwrap();
    assert!(installed.is_some(), "Manifest should be restored");

    assert!(
        shim_path.exists(),
        "Shim should be restored during rollback"
    );
}
