//! Integration tests for frozen lockfile verification and management.

use tempfile::tempdir;
use zoi::cli::InstallScope;
use zoi::cmd;
use zoi::pkg::plugin::PluginManager;
use zoi::pkg::types::{
    DependenciesV2, LockImportV2, LockManifestV2, LockPackageDetailV2,
    LockPackageSourceV2, LockProjectV2, LockRequirementV2, ZoiLockV2
};
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
            epoch: 0,
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
            ..Default::default()
        }
    );
    lock.installed_packages.insert(
        "@community/tools/fd:docs".to_string(),
        LockPackageDetailV2 {
            name: "fd".to_string(),
            sub_package: Some("docs".to_string()),
            repo: "community/tools".to_string(),
            repo_type: "community".to_string(),
            version: "9.0.0".to_string(),
            epoch: 0,
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
            ..Default::default()
        }
    );

    let mut sources = lockfile::sources_from_lock(&lock);
    sources.sort();

    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources.first().expect("Value should exist in test"),
        "#zoidberg@community/tools/fd:docs@9.0.0"
    );
    assert_eq!(
        sources.get(1).expect("Value should exist in test"),
        "#zoidberg@core/hello@1.2.3"
    );
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
            epoch: 0,
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
                test: vec![]
            }),
            ..Default::default()
        }
    );
    lock.installed_packages.insert(
        "@core/lib".to_string(),
        LockPackageDetailV2 {
            name: "lib".to_string(),
            sub_package: None,
            repo: "core".to_string(),
            repo_type: "official".to_string(),
            version: "2.0.0".to_string(),
            epoch: 0,
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
            ..Default::default()
        }
    );

    let locked = lockfile::locked_packages(&lock);
    assert_eq!(locked.len(), 2);

    let app = locked
        .iter()
        .find(|entry| entry.source == "#zoidberg@core/app@1.0.0")
        .expect("app entry should exist");
    assert!(app.direct);
    assert_eq!(
        app.dependencies.as_ref().expect("unwrap failed").runtime,
        vec!["zoi:@core/lib@2.0.0"]
    );

    let lib = locked
        .iter()
        .find(|entry| entry.source == "#zoidberg@core/lib@2.0.0")
        .expect("lib entry should exist");
    assert!(!lib.direct);
}

#[test]
fn test_complete_lockfile_round_trip_preserves_reproducibility_state() {
    let source = LockPackageSourceV2 {
        kind: "git".to_string(),
        request: "gl:zillowe/hello:hello".to_string(),
        url: Some("https://gitlab.com/zillowe/hello.git".to_string()),
        revision: Some("0123456789abcdef".to_string()),
        path: Some("hello.pkg.lua".to_string()),
        definition_hash: Some("sha512-definition".to_string())
    };
    let mut detail = LockPackageDetailV2 {
        name: "hello".to_string(),
        sub_package: None,
        repo: "community".to_string(),
        repo_type: "community".to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        registry: "zoidberg".to_string(),
        why: "direct".to_string(),
        description: "Hello".to_string(),
        package_type_install: "package".to_string(),
        install_method: "source".to_string(),
        installed_sub_packages: Vec::new(),
        platform: "linux-amd64".to_string(),
        hash: "sha512-package".to_string(),
        dependencies: None,
        source: Some(source.clone()),
        definition_hash: source.definition_hash.clone(),
        resolved_dependencies: vec!["lib@1.0.0".to_string()],
        chosen_options: vec!["docs".to_string()],
        chosen_optionals: vec!["tests".to_string()],
        git_sha: source.revision.clone(),
        required_by: Vec::new()
    };
    detail.required_by.push("app@1.0.0".to_string());
    let lock = ZoiLockV2 {
        version: "2".to_string(),
        packages_hash: Some("sha512-store".to_string()),
        registries_hash: Some("sha512-db".to_string()),
        registries: std::collections::BTreeMap::default(),
        installed_packages: [("@community/hello".to_string(), detail)]
            .into_iter()
            .collect(),
        platform: Some("linux-amd64".to_string()),
        manifest: Some(LockManifestV2 {
            path: "zoi.lua".to_string(),
            hash: "sha512-manifest".to_string()
        }),
        project: Some(LockProjectV2 {
            manifest_hash: "sha512-manifest".to_string(),
            tasks_hash: "sha512-tasks".to_string(),
            environments_hash: "sha512-environments".to_string(),
            shell_hash: "sha512-shell".to_string(),
            checks_hash: "sha512-checks".to_string(),
            local: true
        }),
        imports: [(
            "base".to_string(),
            LockImportV2 {
                repo: "zillowe/base".to_string(),
                requested_revision: Some("main".to_string()),
                resolved_revision: "0123456789abcdef".to_string(),
                manifest_hash: "sha512-import".to_string(),
                path: Some("nested".to_string()),
                exports: [("hello".to_string(), "./hello.pkg.lua".to_string())]
                    .into_iter()
                    .collect()
            }
        )]
        .into_iter()
        .collect(),
        root_requirements: vec![LockRequirementV2 {
            source: "@community/hello".to_string(),
            declared_by: "packages".to_string(),
            spec: serde_json::json!({ "version": "1.0.0" })
        }]
    };

    let encoded = serde_json::to_string(&lock).expect("lock should serialize");
    let decoded: ZoiLockV2 =
        serde_json::from_str(&encoded).expect("lock should deserialize");
    let package = decoded
        .installed_packages
        .get("@community/hello")
        .expect("package should survive");

    assert_eq!(
        decoded.manifest.as_ref().expect("manifest").hash,
        "sha512-manifest"
    );
    let import = decoded.imports.get("base").expect("import should survive");
    let requirement = decoded
        .root_requirements
        .first()
        .expect("requirement should survive");
    assert_eq!(import.resolved_revision, "0123456789abcdef");
    assert_eq!(requirement.declared_by, "packages");
    assert_eq!(package.source, Some(source));
    assert_eq!(package.chosen_options, vec!["docs"]);
    assert_eq!(package.chosen_optionals, vec!["tests"]);
    assert_eq!(package.required_by, vec!["app@1.0.0"]);
}

#[test]
fn test_install_frozen_rejects_explicit_sources() {
    let plugin_manager =
        PluginManager::new().expect("plugin manager should initialize");

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
        None
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

    let plugin_manager =
        PluginManager::new().expect("plugin manager should initialize");
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
        None
    )
    .expect_err("missing zoi.lock must fail in frozen mode");

    assert!(err.to_string().contains("--frozen requires zoi.lock"));
}
