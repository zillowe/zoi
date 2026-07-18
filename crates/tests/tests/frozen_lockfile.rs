use tempfile::tempdir;
use zoi::cli::InstallScope;
use zoi::cmd;
use zoi::pkg::plugin::PluginManager;
use zoi::pkg::types::{DependenciesV2, LockPackageDetailV2, ZoiLockV2};
use zoi::project::lockfile;

mod common;

#[test]
fn test_sources_from_lock_uses_packages_map() {
    let mut lock = ZoiLockV2 {
        version: "2".to_string(),
        ..Default::default()
    };
    lock.installed_packages.insert(
        "@core/hello".to_string(),
        LockPackageDetailV2 {
            name: "hello".to_string(),
            sub_package: None,
            repo: "core".to_string(),
            repo_type: "official".to_string(),
            version: "1.2.3".to_string(),
            revision: "1".to_string(),
            registry: "zoidberg".to_string(),
            why: "direct".to_string(),
            description: "Description".to_string(),
            package_type_install: "pre-compiled".to_string(),
            install_method: "pre-built".to_string(),
            installed_sub_packages: vec![],
            platform: "linux-amd64".to_string(),
            hash: "abc".to_string(),
            dependencies: None,
        },
    );
    lock.installed_packages.insert(
        "@community/tools/fd:docs".to_string(),
        LockPackageDetailV2 {
            name: "fd".to_string(),
            sub_package: Some("docs".to_string()),
            repo: "community/tools".to_string(),
            repo_type: "community".to_string(),
            version: "9.0.0".to_string(),
            revision: "1".to_string(),
            registry: "zoidberg".to_string(),
            why: "direct".to_string(),
            description: "Description".to_string(),
            package_type_install: "pre-compiled".to_string(),
            install_method: "pre-built".to_string(),
            installed_sub_packages: vec!["docs".to_string()],
            platform: "linux-amd64".to_string(),
            hash: "def".to_string(),
            dependencies: None,
        },
    );

    let mut sources = lockfile::sources_from_lock(&lock);
    sources.sort();

    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0], "@community/tools/fd:docs@9.0.0");
    assert_eq!(sources[1], "@core/hello@1.2.3");
}

#[test]
fn test_locked_packages_preserve_direct_flags_and_metadata() {
    let mut lock = ZoiLockV2 {
        version: "2".to_string(),
        ..Default::default()
    };

    lock.installed_packages.insert(
        "@core/app".to_string(),
        LockPackageDetailV2 {
            name: "app".to_string(),
            sub_package: None,
            repo: "core".to_string(),
            repo_type: "official".to_string(),
            version: "1.0.0".to_string(),
            revision: "1".to_string(),
            registry: "zoidberg".to_string(),
            why: "direct".to_string(),
            description: "Description".to_string(),
            package_type_install: "pre-compiled".to_string(),
            install_method: "pre-built".to_string(),
            installed_sub_packages: vec![],
            platform: "linux-amd64".to_string(),
            hash: "abc".to_string(),
            dependencies: Some(DependenciesV2 {
                runtime: vec!["zoi:@core/lib@2.0.0".to_string()],
                build: vec![],
            }),
        },
    );
    lock.installed_packages.insert(
        "@core/lib".to_string(),
        LockPackageDetailV2 {
            name: "lib".to_string(),
            sub_package: None,
            repo: "core".to_string(),
            repo_type: "official".to_string(),
            version: "2.0.0".to_string(),
            revision: "1".to_string(),
            registry: "zoidberg".to_string(),
            why: "dependency".to_string(),
            description: "Description".to_string(),
            package_type_install: "pre-compiled".to_string(),
            install_method: "pre-built".to_string(),
            installed_sub_packages: vec![],
            platform: "linux-amd64".to_string(),
            hash: "def".to_string(),
            dependencies: None,
        },
    );

    let locked = lockfile::locked_packages(&lock);
    assert_eq!(locked.len(), 2);

    let app = locked
        .iter()
        .find(|entry| entry.source == "@core/app@1.0.0")
        .expect("app entry should exist");
    assert!(app.direct);
    assert_eq!(
        app.dependencies.as_ref().unwrap().runtime,
        vec!["zoi:@core/lib@2.0.0"]
    );

    let lib = locked
        .iter()
        .find(|entry| entry.source == "@core/lib@2.0.0")
        .expect("lib entry should exist");
    assert!(!lib.direct);
}

#[test]
fn test_install_frozen_rejects_explicit_sources() {
    let plugin_manager = PluginManager::new().expect("plugin manager should initialize");

    let err = cmd::install::run(
        &["hello".to_string()],
        None,
        false,
        false,
        true,
        Some(InstallScope::Project),
        true,
        false,
        false,
        None,
        true,
        Some(&plugin_manager),
        false,
        true,
        false,
        false,
        3,
        false,
        false,
    )
    .expect_err("frozen mode with explicit source must fail");

    assert!(
        err.to_string()
            .contains("--frozen can only be used without explicit sources")
    );
}

#[test]
fn test_install_frozen_requires_zoi_lock() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    ctx.set_current_dir(tmp.path());
    std::fs::write(tmp.path().join("zoi.lua"), "project({ name = 'test' })\n")
        .expect("zoi.lua should be created");

    let plugin_manager = PluginManager::new().expect("plugin manager should initialize");
    let err = cmd::install::run(
        &[],
        None,
        false,
        false,
        true,
        Some(InstallScope::Project),
        true,
        false,
        false,
        None,
        true,
        Some(&plugin_manager),
        false,
        true,
        false,
        false,
        3,
        false,
        false,
    )
    .expect_err("missing zoi.lock must fail in frozen mode");

    assert!(err.to_string().contains("--frozen requires zoi.lock"));
}
