//! Integration tests for package transaction rollback functionality.

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
    common::TestContextGuard::set_sysroot(root.clone());

    let pkg_name = "test-pkg";
    let version = "1.0.0";
    let handle = "local";
    let repo = "test";

    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        handle,
        repo,
        pkg_name,
        version
    )
    .expect("unwrap failed");
    fs::create_dir_all(&version_dir).expect("unwrap failed");

    let manifest = types::InstallManifest {
        name: "test-pkg".to_string(),
        version: version.to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: repo.to_string(),
        repo_type: "official".to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        description: String::new(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
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
        installed_files: vec![],
        installed_size: None,
        sandbox: None,
        completions: None
    };

    let manifest_path = version_dir.join("manifest.yaml");
    fs::write(
        &manifest_path,
        serde_yaml::to_string(&manifest).expect("unwrap failed")
    )
    .expect("unwrap failed");

    let mut trans = transaction::begin().expect("unwrap failed");
    let id = trans.id.clone();
    transaction::record_operation(
        &mut trans,
        types::TransactionOperation::Uninstall {
            manifest: Box::new(manifest)
        }
    )
    .expect("unwrap failed");

    transaction::rollback(&id).expect("unwrap failed");

    let installed =
        local::is_package_installed(pkg_name, None, types::Scope::User)
            .expect("unwrap failed");
    assert!(installed.is_some());
    assert_eq!(installed.expect("unwrap failed").version, version);
}

#[test]
fn test_package_rollback_requires_explicit_source_for_ambiguous_name_matches() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let base_manifest = types::InstallManifest {
        name: "shared".to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: String::new(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
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
        installed_files: vec![],
        installed_size: None,
        sandbox: None,
        completions: None
    };

    let mut extra_manifest = base_manifest.clone();
    extra_manifest.repo = "extra".to_string();

    local::write_manifest(&base_manifest).expect("unwrap failed");
    local::write_manifest(&extra_manifest).expect("unwrap failed");

    let err = rollback::run("shared", true).expect_err("unwrap_err failed");
    assert!(err.to_string().contains("ambiguous"));
}
