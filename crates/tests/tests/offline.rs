//! Integration tests for offline mode and package directory management.

use std::fs;
use tempfile::tempdir;
use zoi::pkg::{offline, pkgdir};
use zoi::utils;

mod common;

#[test]
fn test_offline_mode_toggle() {
    let _ctx = common::TestContextGuard::acquire();
    common::TestContextGuard::set_offline(true);
    assert!(offline::is_offline());

    common::TestContextGuard::set_offline(false);
    assert!(!offline::is_offline());
}

#[test]
fn test_http_client_blocked_in_offline() {
    let _ctx = common::TestContextGuard::acquire();
    common::TestContextGuard::set_offline(true);
    let client = utils::get_http_client();
    assert!(
        client.is_err(),
        "HTTP client should not be created in offline mode"
    );
}

#[test]
fn test_pkg_dirs_can_be_replaced() {
    let _ctx = common::TestContextGuard::acquire();
    let first = tempdir().expect("unwrap failed");
    let second = tempdir().expect("unwrap failed");
    let filename = "archive.zpa";

    fs::write(first.path().join(filename), "first").expect("unwrap failed");
    fs::write(second.path().join(filename), "second").expect("unwrap failed");

    common::TestContextGuard::set_pkg_dirs(vec![first.path().to_path_buf()]);
    assert_eq!(
        pkgdir::find_in_pkg_dirs(filename).expect("unwrap failed"),
        first.path().join(filename)
    );

    common::TestContextGuard::set_pkg_dirs(vec![second.path().to_path_buf()]);
    assert_eq!(
        pkgdir::find_in_pkg_dirs(filename).expect("unwrap failed"),
        second.path().join(filename)
    );
}
