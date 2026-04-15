use std::path::PathBuf;
use tempfile::tempdir;
use zoi::pkg::{install, types};

mod common;

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
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

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
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

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
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

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
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

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
