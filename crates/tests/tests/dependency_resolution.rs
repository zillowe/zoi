//! Integration tests for PubGrub-based dependency resolution.

use rustc_hash::FxHashMap;
use zoi::pkg::install::pubgrub;
use zoi::pkg::install::pubgrub::PkgName;

#[test]
fn test_semver_to_range_exact() {
    let range = pubgrub::semver_to_range("1.2.3");
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.2.3").expect("unwrap failed")
    ));
    assert!(!range.contains(
        &pubgrub::SemVersion::parse("1.2.4").expect("unwrap failed")
    ));
}

#[test]
fn test_semver_to_range_caret() {
    let range = pubgrub::semver_to_range("^1.2.3");
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.2.3").expect("unwrap failed")
    ));
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.9.9").expect("unwrap failed")
    ));
    assert!(!range.contains(
        &pubgrub::SemVersion::parse("2.0.0").expect("unwrap failed")
    ));
}

#[test]
fn test_semver_to_range_tilde() {
    let range = pubgrub::semver_to_range("~1.2.3");
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.2.3").expect("unwrap failed")
    ));
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.2.9").expect("unwrap failed")
    ));
    assert!(!range.contains(
        &pubgrub::SemVersion::parse("1.3.0").expect("unwrap failed")
    ));
}

#[test]
fn test_semver_to_range_comparison() {
    let range = pubgrub::semver_to_range(">=1.0.0, <2.0.0");
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.0.0").expect("unwrap failed")
    ));
    assert!(range.contains(
        &pubgrub::SemVersion::parse("1.5.0").expect("unwrap failed")
    ));
    assert!(!range.contains(
        &pubgrub::SemVersion::parse("2.0.0").expect("unwrap failed")
    ));
}

#[test]
fn test_get_versions_does_not_leak_across_distinct_explicit_sources() {
    let source_a = format!(
        "{}/tests/assets/source_a/shared.pkg.lua@1.0.0",
        env!("CARGO_MANIFEST_DIR")
    );
    let source_b = format!(
        "{}/tests/assets/source_b/shared.pkg.lua@2.0.0",
        env!("CARGO_MANIFEST_DIR")
    );

    let provider = pubgrub::ZoiDependencyProvider::new(
        FxHashMap::default(),
        vec![source_a.clone(), source_b],
        None,
        true,
        true,
        false,
        None,
        None
    )
    .expect("provider should be created");

    let versions = provider
        .get_versions(&PkgName {
            name: "shared".to_string(),
            sub_package: None,
            repo: String::new(),
            registry: "local".to_string(),
            explicit_source: Some(source_a)
        })
        .expect("versions should resolve");

    assert_eq!(versions.len(), 1);
    assert_eq!(
        versions.first().expect("Value should exist in test"),
        &pubgrub::SemVersion::parse("1.0.0").expect("unwrap failed")
    );
}

#[test]
fn git_snapshot_versions_parse_and_order() {
    // Zoidberg's VCS snapshots carry several `+` sections (glibc, gcc). They
    // used to fail to parse, making every package that depends on them
    // unsolvable.
    for v in [
        "2.44+r50+g1848099f063e",
        "16.2.1+r23+gd564253eb6c8",
        "7.1.4.arch1",
        "1:2.44+r50+g1848099f063e"
    ] {
        assert!(pubgrub::SemVersion::parse(v).is_ok(), "failed to parse {v}");
    }
    let parse =
        |v: &str| pubgrub::SemVersion::parse(v).expect("version parses");
    assert!(parse("2.44+r50+g1848099f063e") < parse("2.45"));
    assert!(parse("16.2.1+r23+gd564253eb6c8") > parse("16.2.0"));
    assert_eq!(
        parse("2.44+r50+g1848099f063e").to_string(),
        "2.44+r50+g1848099f063e"
    );
}
