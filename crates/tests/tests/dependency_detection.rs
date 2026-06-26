use std::collections::HashSet;
use std::sync::Mutex;
use zoi::pkg::dependencies::{install_dependency, parse_dependency_string};
use zoi::pkg::types::Scope;

#[test]
fn test_parse_zoi_dependency_channel_version() {
    let dep = parse_dependency_string("zoi:my-pkg@stable").expect("should parse zoi channel dep");

    assert_eq!(dep.manager, "zoi");
    assert_eq!(dep.package, "my-pkg");
    assert_eq!(dep.version_str, Some("stable".to_string()));
    assert!(
        dep.req.is_none(),
        "channel should not be forced through semver parsing"
    );
}

#[test]
fn test_parse_zoi_dependency_prerelease_channel_version() {
    let dep = parse_dependency_string("zoi:my-pkg@alpha").expect("should parse zoi alpha dep");

    assert_eq!(dep.manager, "zoi");
    assert_eq!(dep.package, "my-pkg");
    assert_eq!(dep.version_str, Some("alpha".to_string()));
    assert!(
        dep.req.is_none(),
        "channel should not be forced through semver parsing"
    );
}

#[test]
fn test_parse_zoi_dependency_exact_version() {
    let dep = parse_dependency_string("zoi:my-pkg@1.2.3").expect("should parse zoi exact dep");

    assert_eq!(dep.manager, "zoi");
    assert_eq!(dep.package, "my-pkg");
    assert_eq!(dep.version_str, Some("1.2.3".to_string()));
    assert!(
        dep.req.is_some(),
        "exact version should still produce a semver requirement"
    );
}

#[test]
fn test_skip_missing_package_manager() {
    let dep_str = "apk:some-pkg";
    let dep = parse_dependency_string(dep_str).expect("Failed to parse dependency string");

    assert_eq!(dep.manager, "apk");

    let processed = Mutex::new(HashSet::new());
    let mut installed = Vec::new();

    let result = install_dependency(
        &dep,
        "test-parent",
        Scope::User,
        true,
        true,
        &processed,
        &mut installed,
        None,
    );

    if !zoi::utils::command_exists("apk") {
        assert!(
            result.is_ok(),
            "Should skip missing package manager gracefully"
        );
        assert!(
            installed.contains(&"apk:some-pkg".to_string()),
            "Should still mark as processed/installed to avoid loops"
        );
    }
}
