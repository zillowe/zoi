use semver::Version;
use zoi::pkg::install::pubgrub;

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
