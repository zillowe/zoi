use zoi::pkg::purl::resolve_purl;

#[test]
fn test_purl_missing_repo() {
    let result = resolve_purl("pkg:zoi/zoidberg/hello@4.0.0");
    assert!(result.is_err(), "Expected resolve_purl to fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("PURL missing repository path"),
        "Expected error message for missing repo. stderr: {}",
        err
    );
}

#[test]
fn test_purl_repo_mismatch() {
    let result = resolve_purl("pkg:zoi/zoidberg/asadasd/hello@4.0.0");
    assert!(result.is_err(), "Expected resolve_purl to fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Repository mismatch in PURL") || err.contains("not found in registry"),
        "Expected error message for repo mismatch. err: {}",
        err
    );
}

#[test]
fn test_purl_unsupported_type() {
    let result = resolve_purl("pkg:npm/chalk@4.0.0");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Unsupported PURL type")
    );
}
