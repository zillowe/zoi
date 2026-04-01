use std::collections::HashMap;
use tempfile::tempdir;
use zoi::cli::InstallScope;
use zoi::cmd;
use zoi::pkg::plugin::PluginManager;
use zoi::pkg::types::{LockPackageDetail, ZoiLock};
use zoi::project::lockfile;

#[test]
fn test_sources_from_lock_uses_packages_map() {
    let mut lock = ZoiLock {
        version: "1".to_string(),
        ..Default::default()
    };
    lock.packages
        .insert("#zoidberg@core/hello".to_string(), "1.2.3".to_string());
    lock.packages.insert(
        "#zoidberg@community/tools/fd:docs".to_string(),
        "9.0.0".to_string(),
    );

    let mut sources = lockfile::sources_from_lock(&lock);
    sources.sort();

    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0], "#zoidberg@community/tools/fd:docs@9.0.0");
    assert_eq!(sources[1], "#zoidberg@core/hello@1.2.3");
}

#[test]
fn test_sources_from_lock_falls_back_to_details_when_packages_empty() {
    let mut lock = ZoiLock {
        version: "1".to_string(),
        ..Default::default()
    };
    let mut reg_details = HashMap::new();
    reg_details.insert(
        "@core/hello".to_string(),
        LockPackageDetail {
            version: "2.0.0".to_string(),
            sub_package: None,
            integrity: "abc".to_string(),
            git_sha: None,
            dependencies: vec![],
            options_dependencies: vec![],
            optionals_dependencies: vec![],
        },
    );
    lock.details.insert("#zoidberg".to_string(), reg_details);

    let sources = lockfile::sources_from_lock(&lock);
    assert_eq!(sources, vec!["#zoidberg@core/hello@2.0.0".to_string()]);
}

#[test]
fn test_install_frozen_lockfile_rejects_explicit_sources() {
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
        &plugin_manager,
        false,
        true,
        false,
        false,
        3,
        false,
    )
    .expect_err("frozen lockfile with explicit source must fail");

    assert!(
        err.to_string()
            .contains("--frozen-lockfile can only be used without explicit sources")
    );
}

#[test]
fn test_install_frozen_lockfile_requires_zoi_lock() {
    let tmp = tempdir().expect("tempdir should be created");
    let old_dir = std::env::current_dir().expect("cwd should be readable");
    std::env::set_current_dir(tmp.path()).expect("should enter temp cwd");
    std::fs::write(
        tmp.path().join("zoi.yaml"),
        "name: test\npkgs:\n  - hello\n",
    )
    .expect("zoi.yaml should be created");

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
        &plugin_manager,
        false,
        true,
        false,
        false,
        3,
        false,
    )
    .expect_err("missing zoi.lock must fail in frozen mode");

    std::env::set_current_dir(old_dir).expect("should restore cwd");

    assert!(
        err.to_string()
            .contains("--frozen-lockfile requires zoi.lock")
    );
}
