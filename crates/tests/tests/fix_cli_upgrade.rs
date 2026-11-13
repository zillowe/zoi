//! `zoi upgrade` recognizes a `zoi` binary that is managed by Zoi itself.

use std::fs;

use tempfile::tempdir;
use zoi::cmd::upgrade::find_managing_package;
use zoi::pkg::{local, types};

mod common;

fn manifest(name: &str, files: Vec<String>) -> types::InstallManifest {
    types::InstallManifest {
        name: name.to_string(),
        version: "1.30.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "zoidberg".to_string(),
        package_type: types::PackageType::Package,
        description: "test".to_string(),
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
        platform: String::new(),
        service: None,
        installed_files: files,
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

/// Creates the store binary `${pkgstore}/bin/zoi` for the `zoi` manifest and
/// returns the temporary home (keep it alive) and the binary's path.
fn install_zoi_binary(
    ctx: &mut common::TestContextGuard
) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("HOME", tmp.path());
    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        "zoidberg",
        "core",
        "zoi",
        "1.30.0"
    )
    .expect("version dir");
    let bin = version_dir.join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    let exe = bin.join("zoi");
    fs::write(&exe, "binary").expect("write binary");
    (tmp, exe)
}

#[test]
fn zoi_binary_owned_by_an_installed_package_is_detected() {
    let mut ctx = common::TestContextGuard::acquire();
    let (_home, exe) = install_zoi_binary(&mut ctx);
    let installed = [manifest("zoi", vec!["${pkgstore}/bin/zoi".to_string()])];

    let managed =
        find_managing_package(&exe, &installed).expect("zoi is Zoi-managed");
    assert_eq!(managed.source, "@core/zoi");
    assert_eq!(managed.scope, types::Scope::User);
    assert_eq!(managed.daemon_source, None);
}

#[test]
fn installed_daemon_is_updated_together_with_zoi() {
    let mut ctx = common::TestContextGuard::acquire();
    let (_home, exe) = install_zoi_binary(&mut ctx);
    let installed = [
        manifest("zoi", vec!["${pkgstore}/bin/zoi".to_string()]),
        manifest("zoid", vec!["${pkgstore}/bin/zoid".to_string()])
    ];

    let managed =
        find_managing_package(&exe, &installed).expect("zoi is Zoi-managed");
    assert_eq!(managed.daemon_source.as_deref(), Some("@core/zoid"));
}

#[test]
fn standalone_binary_is_not_zoi_managed() {
    let mut ctx = common::TestContextGuard::acquire();
    let (_home, _exe) = install_zoi_binary(&mut ctx);
    let other = tempdir().expect("tempdir");
    let standalone = other.path().join("zoi");
    fs::write(&standalone, "binary").expect("write binary");
    let installed = [manifest("zoi", vec!["${pkgstore}/bin/zoi".to_string()])];

    assert_eq!(find_managing_package(&standalone, &installed), None);
}
