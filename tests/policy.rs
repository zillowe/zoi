use zoi::pkg::install::resolver::{DependencyGraph, InstallNode};
use zoi::pkg::install::util::check_policy_compliance_with_policy;
use zoi::pkg::types::{InstallReason, Package, Policy};

fn graph_with_package(
    name: &str,
    repo: &str,
    license: &str,
    sub_package: Option<&str>,
) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let sub_package_owned = sub_package.map(|s| s.to_string());
    let id = if let Some(sub) = sub_package {
        format!("{}@1.0.0:{}", name, sub)
    } else {
        format!("{}@1.0.0", name)
    };

    graph.nodes.insert(
        id,
        InstallNode {
            pkg: Package {
                name: name.to_string(),
                repo: repo.to_string(),
                license: license.to_string(),
                sub_package: sub_package_owned.clone(),
                ..Default::default()
            },
            version: "1.0.0".to_string(),
            sub_package: sub_package_owned,
            reason: InstallReason::Direct,
            source: "test.pkg.lua".to_string(),
            registry_handle: "zoidberg".to_string(),
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
            dependencies: Vec::new(),
            git_sha: None,
        },
    );

    graph
}

#[test]
fn blocks_denied_package() {
    let graph = graph_with_package("hello", "core", "MIT", None);
    let policy = Policy {
        denied_packages: Some(vec!["hello".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn allows_matching_allowed_package_with_subpackage() {
    let graph = graph_with_package("hello", "core", "MIT", Some("docs"));
    let policy = Policy {
        allowed_packages: Some(vec!["@core/hello:docs".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn blocks_package_not_in_allowlist() {
    let graph = graph_with_package("hello", "core", "MIT", None);
    let policy = Policy {
        allowed_packages: Some(vec!["other".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn blocks_denied_repo_segment() {
    let graph = graph_with_package("hello", "community/editors", "MIT", None);
    let policy = Policy {
        denied_repos: Some(vec!["community".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn enforces_allowed_repo_exact_path() {
    let graph = graph_with_package("hello", "core/tools", "MIT", None);
    let policy = Policy {
        allowed_repos: Some(vec!["core/tools".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn blocks_denied_license_in_expression() {
    let graph = graph_with_package("hello", "core", "MIT OR GPL-3.0-only", None);
    let policy = Policy {
        denied_licenses: Some(vec!["GPL-3.0-only".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn allows_expression_when_one_license_is_allowed() {
    let graph = graph_with_package("hello", "core", "MIT OR GPL-3.0-only", None);
    let policy = Policy {
        allowed_licenses: Some(vec!["MIT".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn blocks_license_not_in_allowlist() {
    let graph = graph_with_package("hello", "core", "MIT", None);
    let policy = Policy {
        allowed_licenses: Some(vec!["Apache-2.0".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}
