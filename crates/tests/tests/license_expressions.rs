//! Integration tests for SPDX license expression evaluation.

use zoi::pkg::install::resolver::{DependencyGraph, InstallNode};
use zoi::pkg::install::util::check_policy_compliance_with_policy;
use zoi::pkg::types::{InstallReason, Package, Policy};

fn graph_with_license(license: &str) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let id = "test@1.0.0";

    graph.nodes.insert(
        id.to_string(),
        InstallNode {
            description: String::new(),
            repo_type: "official".to_string(),
            pkg: Package {
                name: "test".to_string(),
                repo: "core".to_string(),
                license: license.to_string(),
                ..Default::default()
            },
            version: "1.0.0".to_string(),
            revision: "1".to_string(),
            sub_package: None,
            reason: InstallReason::Direct,
            source: "test.pkg.lua".to_string(),
            registry_handle: "zoidberg".to_string(),
            chosen_options: Vec::new(),
            chosen_optionals: Vec::new(),
            dependencies: Vec::new(),
            git_sha: None
        }
    );

    graph
}

#[test]
fn denied_policy_with_or_expression() {
    // MIT OR GPL-3.0-only. One is denied, but one is NOT.
    // The package should NOT be blocked because it can be used under MIT.
    let graph = graph_with_license("MIT OR GPL-3.0-only");
    let policy = Policy {
        denied_licenses: Some(vec!["GPL-3.0-only".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn denied_policy_with_and_expression() {
    // MIT AND GPL-3.0-only. One is denied.
    // The package SHOULD be blocked because BOTH must be accepted.
    let graph = graph_with_license("MIT AND GPL-3.0-only");
    let policy = Policy {
        denied_licenses: Some(vec!["GPL-3.0-only".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn allowed_policy_with_or_expression() {
    // MIT OR GPL-3.0-only. Only MIT is allowed.
    // The package should be allowed because it can be satisfied by MIT.
    let graph = graph_with_license("MIT OR GPL-3.0-only");
    let policy = Policy {
        allowed_licenses: Some(vec!["MIT".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn allowed_policy_with_and_expression() {
    // MIT AND GPL-3.0-only. Only MIT is allowed.
    // The package should be BLOCKED because GPL-3.0-only is also required.
    let graph = graph_with_license("MIT AND GPL-3.0-only");
    let policy = Policy {
        allowed_licenses: Some(vec!["MIT".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}

#[test]
fn complex_expression_with_exceptions() {
    // Valid SPDX with exception
    let graph = graph_with_license("GPL-3.0-only WITH LLVM-exception");
    let policy = Policy {
        allowed_licenses: Some(vec!["GPL-3.0-only".to_string()]),
        ..Default::default()
    };

    // spdx crate evaluate() handles WITH by checking the base license.
    assert!(check_policy_compliance_with_policy(&graph, &policy).is_ok());
}

#[test]
fn fallback_to_tokenization_for_non_spdx() {
    // "Proprietary" is not a valid SPDX ID.
    let graph = graph_with_license("Proprietary");
    let policy = Policy {
        denied_licenses: Some(vec!["proprietary".to_string()]),
        ..Default::default()
    };

    assert!(check_policy_compliance_with_policy(&graph, &policy).is_err());
}
