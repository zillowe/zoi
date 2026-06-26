use zoi::cmd::ux::{InstallOrigin, classify_source_origin, with_failure_hint};

#[test]
fn classify_source_origin_detects_local_archive() {
    let origin = classify_source_origin("/tmp/demo.pkg.tar.zst", "archive");
    assert_eq!(origin, InstallOrigin::LocalArchive);
}

#[test]
fn classify_source_origin_detects_registry_build() {
    let origin = classify_source_origin("@core/demo", "build");
    assert_eq!(origin, InstallOrigin::RegistrySource);
}

#[test]
fn failure_hint_added_for_policy_error() {
    let err = anyhow::anyhow!("Installation blocked by security/compliance policy.");
    let hinted = with_failure_hint("install", err).to_string();
    assert!(hinted.contains("Hint:"));
    assert!(hinted.contains("policy"));
}
