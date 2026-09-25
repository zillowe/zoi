//! Integration tests for local package installation and resolution.

use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use zoi::pkg::{install, types};

mod common;

fn test_pkg_source() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets")
        .join("test.pkg.lua")
        .to_string_lossy()
        .to_string()
}

fn test_channels_source() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets")
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
    common::TestContextGuard::set_sysroot(root.clone());

    let source = test_pkg_source();
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
        None
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
    common::TestContextGuard::set_sysroot(root.clone());

    let source = format!("{}@1.0.0", test_pkg_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
        None
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
            .join("tests/assets")
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
    common::TestContextGuard::set_sysroot(root.clone());

    let source = format!("{}@stable", test_channels_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
        None
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
    common::TestContextGuard::set_sysroot(root.clone());

    let source = format!("{}@alpha", test_channels_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
        None
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

#[test]
fn resolves_relative_pkg_lua_dependency_from_declaring_file() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    let app = root.join("app.pkg.lua");
    let dependency_dir = root.join("dependency");
    fs::create_dir_all(&dependency_dir)
        .expect("dependency folder should be created");
    fs::write(
        &app,
        r"
metadata({ name = 'relative-app', repo = 'community', version = '1.0.0', description = 'App', maintainer = { name = 'Test', email = 'test@example.com' }, types = { 'source' } })
dependencies({ runtime = { 'zoi:./dependency' } })
",
    )
    .expect("app package should be written");
    fs::write(
        dependency_dir.join("dependency.pkg.lua"),
        "metadata({ name = 'relative-dependency', repo = 'community', version \
         = '1.0.0', description = 'Dependency', maintainer = { name = 'Test', \
         email = 'test@example.com' }, types = { 'source' } })"
    )
    .expect("dependency package should be written");

    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        &[app.to_string_lossy().into_owned()],
        Some(types::Scope::User),
        true,
        true,
        false,
        None,
        true,
        None
    )
    .expect("relative package dependency should resolve");

    assert!(non_zoi_deps.is_empty());
    assert_eq!(graph.nodes.len(), 2);
    assert!(
        graph
            .nodes
            .values()
            .any(|node| node.pkg.name == "relative-app")
    );
    assert!(
        graph
            .nodes
            .values()
            .any(|node| node.pkg.name == "relative-dependency")
    );
}
