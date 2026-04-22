use std::fs;
use tempfile::tempdir;
use zoi::pkg::{local, rollback, transaction, types};

mod common;

#[test]
fn test_transaction_rollback_uninstall() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pkg_name = "test-pkg";
    let version = "1.0.0";
    let handle = "local";
    let repo = "test";

    let version_dir =
        local::get_package_version_dir(types::Scope::User, handle, repo, pkg_name, version)
            .unwrap();
    fs::create_dir_all(&version_dir).unwrap();

    let manifest = types::InstallManifest {
        name: pkg_name.to_string(),
        version: version.to_string(),
        sub_package: None,
        repo: repo.to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        service: None,
        installed_files: vec![],
        installed_size: None,
    };

    let manifest_path = version_dir.join("manifest.yaml");
    fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).unwrap();

    let trans = transaction::begin().unwrap();
    transaction::record_operation(
        &trans.id,
        types::TransactionOperation::Uninstall {
            manifest: Box::new(manifest),
        },
    )
    .unwrap();

    transaction::rollback(&trans.id).unwrap();

    let installed = local::is_package_installed(pkg_name, None, types::Scope::User).unwrap();
    assert!(installed.is_some());
    assert_eq!(installed.unwrap().version, version);
}

#[test]
fn test_package_rollback_requires_explicit_source_for_ambiguous_name_matches() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let base_manifest = types::InstallManifest {
        name: "shared".to_string(),
        version: "1.0.0".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        service: None,
        installed_files: vec![],
        installed_size: None,
    };

    let mut extra_manifest = base_manifest.clone();
    extra_manifest.repo = "extra".to_string();

    local::write_manifest(&base_manifest).unwrap();
    local::write_manifest(&extra_manifest).unwrap();

    let err = rollback::run("shared", true).unwrap_err();
    assert!(err.to_string().contains("ambiguous"));
}
