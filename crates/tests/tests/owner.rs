//! Integration tests for `zoi owner` functionality.

use std::fs;

use tempfile::tempdir;
use zoi::cmd::owner;
use zoi::pkg::{local, types};

mod common;

fn sample_manifest(name: &str, version: &str) -> types::InstallManifest {
    types::InstallManifest {
        name: name.to_string(),
        version: version.to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: "Test package for owner".to_string(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        alternatives: None,
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
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

#[test]
fn test_owner_matches_expanded_pkgstore_file() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    let mut manifest = sample_manifest("test-tool", "1.0.0");
    manifest.installed_files = vec!["${pkgstore}/bin/mytool".to_string()];
    local::write_manifest(&manifest).expect("write manifest");

    let version_dir = local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )
    .expect("version dir");

    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    let tool_file = bin_dir.join("mytool");
    fs::write(&tool_file, "#!/bin/sh\necho ok").expect("write tool");

    // Querying the exact file in the store should succeed
    assert!(owner::run(&tool_file).is_ok());

    // Creating a symlink pointing to the tool should also match
    let symlink_path = root.join("mytool-symlink");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&tool_file, &symlink_path).expect("symlink");
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&tool_file, &symlink_path)
        .expect("symlink");

    assert!(owner::run(&symlink_path).is_ok());

    // Querying a nonexistent or non-owned file
    let unowned_file = root.join("random_unowned_file.txt");
    fs::write(&unowned_file, "random").expect("write unowned");
    assert!(owner::run(&unowned_file).is_ok());
}

#[test]
fn test_owner_matches_subpackage() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    let mut manifest = sample_manifest("meta-suite", "2.0.0");
    manifest.sub_package = Some("cli".to_string());
    manifest.installed_files = vec!["${pkgstore}/bin/subcli".to_string()];
    local::write_manifest(&manifest).expect("write manifest");

    let version_dir = local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version
    )
    .expect("version dir");

    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    let tool_file = bin_dir.join("subcli");
    fs::write(&tool_file, "#!/bin/sh\necho sub").expect("write subcli");

    assert!(owner::run(&tool_file).is_ok());
}
