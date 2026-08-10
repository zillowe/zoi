//! Integration tests for security advisory management and checking.

use tempfile::tempdir;
use zoi::pkg::{db, types};

mod common;

#[test]
fn test_advisory_indexing_and_query() {
    let mut ctx = common::TestContextGuard::acquire();
    let dir = tempdir().expect("unwrap failed");
    let db_dir = dir.path().to_path_buf();
    ctx.set_env_var("ZOI_DB_DIR", &db_dir);

    let handle = "test-reg";
    let conn = db::open_connection(handle).expect("unwrap failed");

    let advisory = types::Advisory {
        id: "ZSA-2026-D0001".to_string(),
        package: "test-pkg".to_string(),
        sub_package: None,
        summary: "Critical vulnerability".to_string(),
        severity: types::Severity::Critical,
        cvss: Some("9.8".to_string()),
        affected_range: ">=1.0.0, <1.1.0".to_string(),
        fixed_in: Some("1.1.0".to_string()),
        description: "A test vulnerability".to_string(),
        references: None
    };

    db::update_advisory(&conn, &advisory, "community", handle)
        .expect("unwrap failed");

    let advisories = db::get_advisories_for_package(handle, "test-pkg", None)
        .expect("unwrap failed");

    assert_eq!(advisories.len(), 1);
    assert_eq!(
        advisories.first().expect("Value should exist in test").id,
        "ZSA-2026-D0001"
    );
    assert_eq!(
        advisories
            .first()
            .expect("Value should exist in test")
            .severity,
        types::Severity::Critical
    );
}

#[test]
fn test_sub_package_advisory_filtering() {
    let mut ctx = common::TestContextGuard::acquire();
    let dir = tempdir().expect("unwrap failed");
    let db_dir = dir.path().to_path_buf();
    ctx.set_env_var("ZOI_DB_DIR", &db_dir);

    let handle = "test-reg";
    let conn = db::open_connection(handle).expect("unwrap failed");

    let adv_global = types::Advisory {
        id: "ZSA-2026-C0001".to_string(),
        package: "linux".to_string(),
        sub_package: None,
        summary: "Global kernel bug".to_string(),
        severity: types::Severity::High,
        cvss: None,
        affected_range: ">=5.0.0, <6.0.0".to_string(),
        fixed_in: Some("6.0.0".to_string()),
        description: "Test".to_string(),
        references: None
    };

    let adv_sub = types::Advisory {
        id: "ZSA-2026-A0002".to_string(),
        package: "linux".to_string(),
        sub_package: Some("docs".to_string()),
        summary: "Docs typo exploit".to_string(),
        severity: types::Severity::Low,
        cvss: None,
        affected_range: ">=1.0.0, <9.0.0".to_string(),
        fixed_in: None,
        description: "Test".to_string(),
        references: None
    };

    db::update_advisory(&conn, &adv_global, "core", handle)
        .expect("unwrap failed");
    db::update_advisory(&conn, &adv_sub, "core", handle)
        .expect("unwrap failed");

    let res_base = db::get_advisories_for_package(handle, "linux", None)
        .expect("unwrap failed");
    assert_eq!(res_base.len(), 1);
    assert_eq!(
        res_base.first().expect("Value should exist in test").id,
        "ZSA-2026-C0001"
    );

    let res_docs =
        db::get_advisories_for_package(handle, "linux", Some("docs"))
            .expect("unwrap failed");
    assert_eq!(res_docs.len(), 2);
    let ids: Vec<_> = res_docs.iter().map(|a| &a.id).collect();
    assert!(ids.contains(&&"ZSA-2026-C0001".to_string()));
    assert!(ids.contains(&&"ZSA-2026-A0002".to_string()));

    let res_headers =
        db::get_advisories_for_package(handle, "linux", Some("headers"))
            .expect("unwrap failed");
    assert_eq!(res_headers.len(), 1);
    assert_eq!(
        res_headers.first().expect("Value should exist in test").id,
        "ZSA-2026-C0001"
    );
}

#[test]
fn test_version_range_matching() {
    use semver::{Version, VersionReq};

    let range = ">=1.0.0, <1.1.0";
    let req = VersionReq::parse(range).expect("unwrap failed");

    assert!(req.matches(&Version::parse("1.0.0").expect("unwrap failed")));
    assert!(req.matches(&Version::parse("1.0.5").expect("unwrap failed")));
    assert!(!req.matches(&Version::parse("1.1.0").expect("unwrap failed")));
    assert!(!req.matches(&Version::parse("0.9.9").expect("unwrap failed")));
}
