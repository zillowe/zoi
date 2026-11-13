//! Regression tests for local store state handling: dependent-link
//! matching, corrupt manifests and atomic manifest writes.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::pkg::{local, resolve, types};

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
        description: String::new(),
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

fn isolate(ctx: &mut common::TestContextGuard, root: &Path) {
    ctx.set_env_var("HOME", root);
    ctx.set_env_var("XDG_DATA_HOME", root.join(".local/share"));
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    ctx.set_env_var("XDG_CONFIG_HOME", root.join(".config"));
    ctx.set_env_var("XDG_CACHE_HOME", root.join(".cache"));
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));
    common::TestContextGuard::set_sysroot(root.to_path_buf());
}

fn dependents_of(dir: &Path) -> Vec<String> {
    let mut deps = local::get_dependents(dir).expect("unwrap failed");
    deps.sort();
    deps
}

#[test]
fn test_remove_dependent_matches_sub_package_and_install_graph_shapes() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let dir = tmp.path();

    // Install graph shape (no sub-package) is removed by the `:sub` shape.
    local::add_dependent(dir, "#h@core/parent@1.0.0").expect("unwrap failed");
    local::remove_dependent(dir, "#h@core/parent:dev@1.0.0")
        .expect("unwrap failed");
    assert_eq!(dependents_of(dir), [] as [std::string::String; 0]);

    // `:sub` shape is removed by the exact `:sub` ID.
    local::add_dependent(dir, "#h@core/parent:dev@1.0.0")
        .expect("unwrap failed");
    local::remove_dependent(dir, "#h@core/parent:dev@1.0.0")
        .expect("unwrap failed");
    assert_eq!(dependents_of(dir), [] as [std::string::String; 0]);

    // Legacy unversioned IDs are removed by a versioned ID.
    local::add_dependent(dir, "#h@core/parent").expect("unwrap failed");
    local::remove_dependent(dir, "#h@core/parent@1.0.0")
        .expect("unwrap failed");
    assert_eq!(dependents_of(dir), [] as [std::string::String; 0]);
}

#[test]
fn test_remove_dependent_keeps_unrelated_links() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let dir = tmp.path();

    for id in [
        "#h@core/parent:dev@1.0.0",
        "#h@core/parent@2.0.0",
        "#h@core/parent-two@1.0.0",
        "#h@core/parent:docs@1.0.0"
    ] {
        local::add_dependent(dir, id).expect("unwrap failed");
    }

    // Removing the main package's link must not drop links held by its
    // sub-packages, other versions or similarly named packages.
    local::remove_dependent(dir, "#h@core/parent@1.0.0")
        .expect("unwrap failed");
    // Removing one sub-package's link must not drop another sub-package's.
    local::remove_dependent(dir, "#h@core/parent:other@1.0.0")
        .expect("unwrap failed");

    assert_eq!(
        dependents_of(dir),
        vec![
            "#h@core/parent-two@1.0.0".to_string(),
            "#h@core/parent:dev@1.0.0".to_string(),
            "#h@core/parent:docs@1.0.0".to_string(),
            "#h@core/parent@2.0.0".to_string(),
        ]
    );
}

#[test]
fn test_corrupt_manifest_does_not_block_lookups() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    isolate(&mut ctx, tmp.path());

    let good = sample_manifest("good-pkg", "1.0.0");
    let bad = sample_manifest("bad-pkg", "1.0.0");
    local::write_manifest(&good).expect("unwrap failed");
    local::write_manifest(&bad).expect("unwrap failed");

    let bad_dir = local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        "bad-pkg",
        "1.0.0"
    )
    .expect("unwrap failed");
    fs::write(bad_dir.join("manifest.json"), "{ truncated")
        .expect("unwrap failed");

    let manifests = local::get_installed_manifests_in_scope(types::Scope::User)
        .expect("a corrupt manifest must not fail the whole scan");
    let names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["good-pkg"]);

    let request =
        resolve::parse_source_string("good-pkg").expect("unwrap failed");
    let matches =
        local::find_installed_manifests_matching(&request, types::Scope::User)
            .expect("lookup should succeed despite the corrupt manifest");
    assert_eq!(matches.len(), 1);
}

#[test]
fn test_write_manifest_is_atomic_and_leaves_no_temp_files() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    isolate(&mut ctx, tmp.path());

    let manifest = sample_manifest("atomic-pkg", "1.0.0");
    local::write_manifest(&manifest).expect("unwrap failed");
    // Overwrite an existing manifest as upgrades and reason changes do.
    local::update_manifest_reason(
        &manifest,
        types::InstallReason::Dependency {
            parent: "#local@core/other@1.0.0".to_string()
        }
    )
    .expect("unwrap failed");

    let version_dir = local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        "atomic-pkg",
        "1.0.0"
    )
    .expect("unwrap failed");
    let names: Vec<String> = fs::read_dir(&version_dir)
        .expect("unwrap failed")
        .map(|e| {
            e.expect("unwrap failed")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(names, vec!["manifest.json".to_string()]);

    let read_back =
        local::read_store_manifest(&version_dir.join("manifest.json"))
            .expect("unwrap failed");
    assert!(matches!(
        read_back.reason,
        types::InstallReason::Dependency { .. }
    ));

    let file = tmp.path().join("plain.txt");
    local::write_file_atomic(&file, b"first").expect("unwrap failed");
    local::write_file_atomic(&file, b"second").expect("unwrap failed");
    assert_eq!(fs::read_to_string(&file).expect("unwrap failed"), "second");
}
