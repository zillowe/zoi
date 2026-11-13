//! Parsing of Zoi dependency strings with sub-packages, registries and
//! descriptions.

use zoi::pkg::dependencies::parse_dependency_string;

#[test]
fn zoi_sub_package_is_kept_in_the_package() {
    let dep = parse_dependency_string("zoi:@core/gcc:libgcc").expect("parse");
    assert_eq!(dep.manager, "zoi");
    assert_eq!(dep.package, "@core/gcc:libgcc");
    assert_eq!(dep.version_str, None);
    assert_eq!(dep.description, None);
}

#[test]
fn zoi_sub_package_with_symbols_and_version() {
    let dep = parse_dependency_string("zoi:@core/gcc:libstdc++@16.2.1")
        .expect("parse");
    assert_eq!(dep.package, "@core/gcc:libstdc++");
    assert_eq!(dep.version_str.as_deref(), Some("16.2.1"));
    assert!(dep.req.is_some());
}

#[test]
fn bare_root_package_keeps_its_sub_package() {
    // The default `--pure` root package.
    let dep = parse_dependency_string("@core/base:dev").expect("parse");
    assert_eq!(dep.manager, "zoi");
    assert_eq!(dep.package, "@core/base:dev");
}

#[test]
fn zoi_description_with_spaces_is_not_a_sub_package() {
    let dep = parse_dependency_string(
        "zoi:@core/bubblewrap:for building pure packages"
    )
    .expect("parse");
    assert_eq!(dep.package, "@core/bubblewrap");
    assert_eq!(dep.description, Some("for building pure packages"));
}

#[test]
fn zoi_sub_package_version_and_description() {
    let dep =
        parse_dependency_string("zoi:@core/gcc:libgcc@>=16:compiler runtime")
            .expect("parse");
    assert_eq!(dep.package, "@core/gcc:libgcc");
    assert_eq!(dep.version_str.as_deref(), Some(">=16"));
    assert_eq!(dep.description, Some("compiler runtime"));
}

#[test]
fn zoi_version_ranges_may_contain_spaces() {
    let dep = parse_dependency_string("zoi:foo@>=1.0, <2.0").expect("parse");
    assert_eq!(dep.package, "foo");
    assert_eq!(dep.version_str.as_deref(), Some(">=1.0, <2.0"));
    let req = dep.req.expect("range must parse as a version requirement");
    assert!(req.matches(&semver::Version::new(1, 5, 0)));
    assert!(!req.matches(&semver::Version::new(2, 0, 0)));
}

#[test]
fn zoi_range_with_sub_package_and_description() {
    let dep = parse_dependency_string(
        "zoi:@core/gcc:libgcc@>=16.0, <17.0:compiler runtime"
    )
    .expect("parse");
    assert_eq!(dep.package, "@core/gcc:libgcc");
    assert_eq!(dep.version_str.as_deref(), Some(">=16.0, <17.0"));
    assert_eq!(dep.description, Some("compiler runtime"));
}

#[test]
fn zoi_operator_range_with_spaces() {
    let dep = parse_dependency_string("zoi:foo>=1.0, <2.0").expect("parse");
    assert_eq!(dep.package, "foo");
    assert_eq!(dep.version_str.as_deref(), Some(">=1.0, <2.0"));
    assert!(dep.req.is_some());
}

#[test]
fn recorded_dependency_with_registry_handle() {
    let dep = parse_dependency_string("zoi:#local@core/dep:sub@1.0.0")
        .expect("parse");
    assert_eq!(dep.package, "#local@core/dep:sub");
    assert_eq!(dep.version_str.as_deref(), Some("1.0.0"));
}

#[test]
fn plain_zoi_names_and_channels_are_unchanged() {
    let dep = parse_dependency_string("zoi:my-pkg@stable").expect("parse");
    assert_eq!(dep.package, "my-pkg");
    assert_eq!(dep.version_str.as_deref(), Some("stable"));

    let dep = parse_dependency_string("@core/make").expect("parse");
    assert_eq!(dep.package, "@core/make");
    assert_eq!(dep.version_str, None);
}

#[test]
fn external_manager_descriptions_are_unchanged() {
    let dep = parse_dependency_string("pipx:black:Python formatter CLI")
        .expect("parse");
    assert_eq!(dep.manager, "pipx");
    assert_eq!(dep.package, "black");
    assert_eq!(dep.description, Some("Python formatter CLI"));
}
