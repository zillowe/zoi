use std::collections::HashSet;
use std::sync::Mutex;
use zoi::pkg::dependencies::{install_dependency, parse_dependency_string};
use zoi::pkg::types::Scope;

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
