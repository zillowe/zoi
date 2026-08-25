//! Integration tests for the reinstall guard of the install command.
//!
//! These verify that already-installed packages are reliably detected before
//! an installation proceeds: case-insensitive identifier matching, sub-package
//! awareness, scope isolation, and version spec handling.

use tempfile::{TempDir, tempdir};
use zoi::pkg::{local, resolve};

mod common;

fn sample_manifest(
    name: &str,
    repo: &str,
    handle: &str
) -> zoi::pkg::types::InstallManifest {
    zoi::pkg::types::InstallManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: repo.to_string(),
        repo_type: "official".to_string(),
        registry_handle: handle.to_string(),
        package_type: zoi::pkg::types::PackageType::Package,
        description: String::new(),
        reason: zoi::pkg::types::InstallReason::Direct,
        scope: zoi::pkg::types::Scope::User,
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
    }
}

fn test_context() -> (common::TestContextGuard, TempDir) {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root);
    (ctx, tmp)
}

#[test]
fn matches_installed_manifest_with_mixed_case_repo_and_handle() {
    let (_ctx, _tmp) = test_context();

    // Installed with original casing from package metadata.
    let mut manifest = sample_manifest("gct", "Zillowe", "Zoidberg");
    manifest.version = "1.4.1".to_string();
    local::write_manifest(&manifest).expect("unwrap failed");

    // Requests built from source strings are lowercased by the parser.
    let request =
        resolve::parse_source_string("@zillowe/gct").expect("unwrap failed");
    let found = local::find_installed_manifests_matching(
        &request,
        zoi::pkg::types::Scope::User
    )
    .expect("unwrap failed");

    assert_eq!(found.len(), 1, "mixed-case repo/handle must still match");
    assert_eq!(found.first().map(|m| m.version.as_str()), Some("1.4.1"));
}

#[test]
fn sub_package_request_only_matches_same_sub_package() {
    let (_ctx, _tmp) = test_context();

    let mut manifest = sample_manifest("linux", "core", "local");
    manifest.sub_package = Some("headers".to_string());
    local::write_manifest(&manifest).expect("unwrap failed");

    let sub_request =
        resolve::parse_source_string("linux:headers").expect("unwrap failed");
    let found = local::find_installed_manifests_matching(
        &sub_request,
        zoi::pkg::types::Scope::User
    )
    .expect("unwrap failed");
    assert_eq!(
        found.len(),
        1,
        "sub-package request must match its manifest"
    );

    let plain_request =
        resolve::parse_source_string("linux").expect("unwrap failed");
    let found = local::find_installed_manifests_matching(
        &plain_request,
        zoi::pkg::types::Scope::User
    )
    .expect("unwrap failed");
    assert!(
        found.is_empty(),
        "plain request must not match a sub-package manifest"
    );
}

#[test]
fn installed_package_is_not_visible_from_other_scopes() {
    let (_ctx, _tmp) = test_context();

    let manifest = sample_manifest("gct", "zillowe", "local");
    local::write_manifest(&manifest).expect("unwrap failed");

    let request =
        resolve::parse_source_string("@zillowe/gct").expect("unwrap failed");
    let system_found = local::find_installed_manifests_matching(
        &request,
        zoi::pkg::types::Scope::System
    )
    .expect("unwrap failed");
    assert!(
        system_found.is_empty(),
        "user-scope installs must not leak into the system scope"
    );

    let user_found = local::find_installed_manifests_matching(
        &request,
        zoi::pkg::types::Scope::User
    )
    .expect("unwrap failed");
    assert_eq!(user_found.len(), 1);
}

#[test]
fn version_spec_request_does_not_match_other_versions() {
    let (_ctx, _tmp) = test_context();

    let manifest = sample_manifest("gct", "zillowe", "local");
    local::write_manifest(&manifest).expect("unwrap failed");

    let request = resolve::parse_source_string("@zillowe/gct@2.0.0")
        .expect("unwrap failed");
    let found = local::find_installed_manifests_matching(
        &request,
        zoi::pkg::types::Scope::User
    )
    .expect("unwrap failed");
    assert!(
        found.is_empty(),
        "exact version spec must not match a different installed version"
    );
}
