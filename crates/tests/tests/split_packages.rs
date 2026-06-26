use std::path::PathBuf;
use tempfile::tempdir;
use zoi::pkg::{config, db, install, resolve, types};

mod common;

fn test_split_source() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets")
        .join("test_sub_packages.pkg.lua")
        .to_string_lossy()
        .to_string()
}

#[test]
fn parse_source_with_sub_package() {
    let req = resolve::parse_source_string("linux:headers").unwrap();
    assert_eq!(req.name, "linux");
    assert_eq!(req.sub_package, Some("headers".to_string()));
}

#[test]
fn parse_source_with_sub_package_and_version() {
    let req = resolve::parse_source_string("pkg:sub@1.0.0").unwrap();
    assert_eq!(req.name, "pkg");
    assert_eq!(req.sub_package, Some("sub".to_string()));
    assert_eq!(req.version_spec, Some("1.0.0".to_string()));
}

#[test]
fn parse_source_with_sub_package_full_spec() {
    let req = resolve::parse_source_string("#handle@repo/pkg:sub@1.0.0").unwrap();
    assert_eq!(req.name, "pkg");
    assert_eq!(req.sub_package, Some("sub".to_string()));
    assert_eq!(req.handle, Some("handle".to_string()));
    assert_eq!(req.repo, Some("repo".to_string()));
}

#[test]
fn parse_source_base_package_has_no_sub_package() {
    let req = resolve::parse_source_string("linux").unwrap();
    assert_eq!(req.name, "linux");
    assert_eq!(req.sub_package, None);
}

#[test]
fn resolver_install_node_has_sub_package_from_source() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

    let source = format!("{}:dev", test_split_source());
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("split pkg.lua source with sub should resolve");

    assert!(non_zoi_deps.is_empty());
    assert_eq!(graph.nodes.len(), 1);

    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.pkg.name, "test-split");
    assert_eq!(node.sub_package, Some("dev".to_string()));
    assert_eq!(node.version, "1.0.0");
}

#[test]
fn resolver_install_node_base_has_no_sub_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_sysroot(root.clone());

    let source = test_split_source();
    let (graph, non_zoi_deps) = install::resolver::resolve_dependency_graph(
        std::slice::from_ref(&source),
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
    )
    .expect("split pkg.lua source should resolve");

    assert!(non_zoi_deps.is_empty());
    assert_eq!(graph.nodes.len(), 1);

    let node = graph
        .nodes
        .values()
        .next()
        .expect("graph should contain one node");
    assert_eq!(node.pkg.name, "test-split");
    assert_eq!(node.sub_package, None);
    assert!(node.pkg.sub_packages.is_some());
    let subs = node.pkg.sub_packages.as_ref().unwrap();
    assert!(subs.contains(&"dev".to_string()));
    assert!(subs.contains(&"lib".to_string()));
}

#[test]
fn db_update_and_query_sub_package() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", &root);
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));
    ctx.set_sysroot(root.clone());

    let cfg = types::Config {
        added_registries: vec![types::Registry {
            handle: "testreg".to_string(),
            url: "https://example.com/testreg.git".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let conn = db::open_connection("testreg").unwrap();

    let base_pkg = types::Package {
        name: "split-pkg".to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        description: "Split package test".to_string(),
        sub_packages: Some(vec!["dev".to_string(), "lib".to_string()]),
        ..Default::default()
    };

    db::update_package(&conn, &base_pkg, "testreg", None, None, None).unwrap();
    db::update_package(&conn, &base_pkg, "testreg", None, Some("dev"), None).unwrap();
    db::update_package(&conn, &base_pkg, "testreg", None, Some("lib"), None).unwrap();

    let all_pkgs = db::list_all_packages("testreg").unwrap();
    let split_pkgs: Vec<&types::Package> =
        all_pkgs.iter().filter(|p| p.name == "split-pkg").collect();
    assert_eq!(split_pkgs.len(), 3, "should have base + 2 sub-packages");

    let base = split_pkgs.iter().find(|p| p.sub_package.is_none()).unwrap();
    assert_eq!(base.name, "split-pkg");
    assert_eq!(base.sub_package, None);

    let dev = split_pkgs
        .iter()
        .find(|p| p.sub_package == Some("dev".to_string()))
        .unwrap();
    assert_eq!(dev.sub_package, Some("dev".to_string()));

    let lib = split_pkgs
        .iter()
        .find(|p| p.sub_package == Some("lib".to_string()))
        .unwrap();
    assert_eq!(lib.sub_package, Some("lib".to_string()));
}

#[test]
fn local_file_source_with_sub_package_parses_correctly() {
    let source = format!("{}:dev", test_split_source());
    let req = resolve::parse_source_string(&source).unwrap();
    assert_eq!(req.name, "test-split");
    assert_eq!(req.sub_package, Some("dev".to_string()));
}
