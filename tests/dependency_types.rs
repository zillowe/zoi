use zoi::pkg::types::{ComplexDependencyGroup, DependencyGroup};

#[test]
fn simple_dependency_group_has_no_optional_dependencies() {
    let group = DependencyGroup::Simple(vec!["core/pkg".to_string()]);

    assert!(group.get_optional().is_empty());
}

#[test]
fn complex_dependency_group_returns_optional_dependencies_as_slice() {
    let group = DependencyGroup::Complex(ComplexDependencyGroup {
        optional: vec!["extra/tool".to_string()],
        ..Default::default()
    });
    let expected = vec!["extra/tool".to_string()];

    assert_eq!(group.get_optional(), &expected);
}
