use std::path::PathBuf;
use tempfile::tempdir;
use zoi::pkg::{install, sysroot, types};

fn test_pkg_source() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_assets")
        .join("test.pkg.lua")
        .to_string_lossy()
        .to_string()
}

fn test_channels_source() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_assets")
        .join("test_channels.pkg.lua")
        .to_string_lossy()
        .to_string()
}

#[test]
fn resolves_dependency_graph_for_local_pkg_lua_source_in_test_assets() {
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    unsafe {
        std::env::set_var("HOME", &root);
    }
    sysroot::set_sysroot(root);

    let source = test_pkg_source();
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("local pkg.lua source should resolve");

    assert!(non_zoi_deps.is_empty());
    assert_eq!(graph.nodes.len(), 1);

    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.pkg.name, "test-pkg");
    assert_eq!(node.version, "1.0.0");
    assert_eq!(node.source, source);
}

#[test]
fn resolves_dependency_graph_for_versioned_local_pkg_lua_source() {
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    unsafe {
        std::env::set_var("HOME", &root);
    }
    sysroot::set_sysroot(root);

    let source = format!("{}@1.0.0", test_pkg_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("versioned local pkg.lua source should resolve");

    assert!(non_zoi_deps.is_empty());
    assert_eq!(graph.nodes.len(), 1);

    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.pkg.name, "test-pkg");
    assert_eq!(node.version, "1.0.0");
    assert_eq!(
        node.source,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test_assets")
            .join("test.pkg.lua")
            .to_string_lossy()
            .to_string()
    );
}

#[test]
fn resolves_dependency_graph_for_local_pkg_lua_stable_channel() {
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    unsafe {
        std::env::set_var("HOME", &root);
    }
    sysroot::set_sysroot(root);

    let source = format!("{}@stable", test_channels_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("stable channel local pkg.lua source should resolve");

    assert!(non_zoi_deps.is_empty());
    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.version, "1.0.0");
}

#[test]
fn resolves_dependency_graph_for_local_pkg_lua_alpha_channel() {
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    unsafe {
        std::env::set_var("HOME", &root);
    }
    sysroot::set_sysroot(root);

    let source = format!("{}@alpha", test_channels_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("alpha channel local pkg.lua source should resolve");

    assert!(non_zoi_deps.is_empty());
    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.version, "1.1.0-alpha");
}
