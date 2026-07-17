use std::fs;
use tempfile::tempdir;
use zoi::pkg::utils;

mod common;

#[test]
fn test_is_zoios_detection() {
    let ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_sysroot(root.clone());

    let os_release_dir = root.join("etc");
    fs::create_dir_all(&os_release_dir).unwrap();
    let os_release_path = os_release_dir.join("os-release");

    // Test 1: Not ZoiOS
    fs::write(&os_release_path, "ID=fedora\nID_LIKE=rhel fedora\n").unwrap();
    assert!(!utils::is_zoios());

    // Test 2: Parlex (ID)
    fs::write(&os_release_path, "ID=parlex\n").unwrap();
    assert!(utils::is_zoios());

    // Test 3: ZoiOS (ID)
    fs::write(&os_release_path, "ID=zoios\n").unwrap();
    assert!(utils::is_zoios());

    // Test 4: ZoiOS (ID_LIKE)
    fs::write(
        &os_release_path,
        "ID=custom-distro\nID_LIKE=\"zoios debian\"\n",
    )
    .unwrap();
    assert!(utils::is_zoios());
}

#[test]
fn test_scope_compliance_validation() {
    use zoi::pkg::install::resolver::{DependencyGraph, InstallNode};
    use zoi::pkg::install::util::check_scope_compliance;
    use zoi::pkg::types::{InstallReason, Package, Scope};

    let mut graph = DependencyGraph::default();

    // Package that only allows 'system' scope
    let pkg = Package {
        name: "kernel".to_string(),
        scopes: Some(vec![Scope::System]),
        scope: Scope::User, // Attempting to install in 'user' scope
        ..Default::default()
    };

    graph.nodes.insert(
        "kernel@5.15".to_string(),
        InstallNode {
            pkg,
            version: "5.15".to_string(),
            revision: "1".to_string(),
            sub_package: None,
            repo_type: "official".to_string(),
            description: "test".to_string(),
            reason: InstallReason::Direct,
            source: "local".to_string(),
            registry_handle: "zoidberg".to_string(),
            chosen_options: vec![],
            chosen_optionals: vec![],
            dependencies: vec![],
            git_sha: None,
        },
    );

    let result = check_scope_compliance(&graph);
    assert!(result.is_err(), "Should fail when scope is not allowed");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not allowed to be installed in scope User")
    );
}
