use tempfile::tempdir;
use zoi::pkg::{db, types};

#[test]
fn test_advisory_indexing_and_query() {
    let dir = tempdir().unwrap();
    let db_dir = dir.path().to_path_buf();

    unsafe {
        std::env::set_var("ZOI_DB_DIR", &db_dir);
    }

    let handle = "test-reg";
    let conn = db::open_connection(handle).unwrap();

    let advisory = types::Advisory {
        id: "ZSA-2026-D0001".to_string(),
        package: "test-pkg".to_string(),
        summary: "Critical vulnerability".to_string(),
        severity: types::Severity::Critical,
        cvss: Some("9.8".to_string()),
        affected_range: ">=1.0.0 <1.1.0".to_string(),
        fixed_in: Some("1.1.0".to_string()),
        description: "A test vulnerability".to_string(),
        references: None,
    };

    db::update_advisory(&conn, &advisory, "community", handle).unwrap();

    let advisories = db::get_advisories_for_package(handle, "test-pkg").unwrap();
    assert_eq!(advisories.len(), 1);
    assert_eq!(advisories[0].id, "ZSA-2026-D0001");
    assert_eq!(advisories[0].severity, types::Severity::Critical);

    unsafe {
        std::env::remove_var("ZOI_DB_DIR");
    }
}

#[test]
fn test_version_range_matching() {
    use semver::{Version, VersionReq};

    let range = ">=1.0.0 <1.1.0";
    let req = VersionReq::parse(range).unwrap();

    assert!(req.matches(&Version::parse("1.0.0").unwrap()));
    assert!(req.matches(&Version::parse("1.0.5").unwrap()));
    assert!(!req.matches(&Version::parse("1.1.0").unwrap()));
    assert!(!req.matches(&Version::parse("0.9.9").unwrap()));
}
