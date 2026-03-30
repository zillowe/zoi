use rustc_hash::FxHashMap;
use semver::Version;
use zoi::pkg::install::pubgrub;
use zoi::pkg::install::pubgrub::PkgName;

#[test]
fn test_semver_to_range_exact() {
    let range = pubgrub::semver_to_range("1.2.3");
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.2.3").unwrap())));
    assert!(!range.contains(&pubgrub::SemVersion(Version::parse("1.2.4").unwrap())));
}

#[test]
fn test_semver_to_range_caret() {
    let range = pubgrub::semver_to_range("^1.2.3");
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.2.3").unwrap())));
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.9.9").unwrap())));
    assert!(!range.contains(&pubgrub::SemVersion(Version::parse("2.0.0").unwrap())));
}

#[test]
fn test_semver_to_range_tilde() {
    let range = pubgrub::semver_to_range("~1.2.3");
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.2.3").unwrap())));
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.2.9").unwrap())));
    assert!(!range.contains(&pubgrub::SemVersion(Version::parse("1.3.0").unwrap())));
}

#[test]
fn test_semver_to_range_comparison() {
    let range = pubgrub::semver_to_range(">=1.0.0, <2.0.0");
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.0.0").unwrap())));
    assert!(range.contains(&pubgrub::SemVersion(Version::parse("1.5.0").unwrap())));
    assert!(!range.contains(&pubgrub::SemVersion(Version::parse("2.0.0").unwrap())));
}

#[test]
fn test_get_versions_does_not_leak_across_distinct_explicit_sources() {
    let source_a = format!(
        "{}/test_assets/source_a/shared.pkg.lua@1.0.0",
        env!("CARGO_MANIFEST_DIR")
    );
    let source_b = format!(
        "{}/test_assets/source_b/shared.pkg.lua@2.0.0",
        env!("CARGO_MANIFEST_DIR")
    );

    let provider = pubgrub::ZoiDependencyProvider::new(
        FxHashMap::default(),
        vec![source_a.clone(), source_b],
        true,
        true,
        false,
    )
    .expect("provider should be created");

    let versions = provider
        .get_versions(&PkgName {
            name: "shared".to_string(),
            sub_package: None,
            repo: "".to_string(),
            registry: "local".to_string(),
            explicit_source: Some(source_a),
        })
        .expect("versions should resolve");

    assert_eq!(versions.len(), 1);
    assert_eq!(
        versions[0],
        pubgrub::SemVersion(Version::parse("1.0.0").unwrap())
    );
}
