use std::fs;
use tempfile::tempdir;
use zoi::pkg::{offline, pkgdir};
use zoi::utils;

mod common;

#[test]
fn test_offline_mode_toggle() {
    let ctx = common::TestContextGuard::acquire();
    ctx.set_offline(true);
    assert!(offline::is_offline());

    ctx.set_offline(false);
    assert!(!offline::is_offline());
}

#[test]
fn test_http_client_blocked_in_offline() {
    let ctx = common::TestContextGuard::acquire();
    ctx.set_offline(true);
    let client = utils::get_http_client();
    assert!(
        client.is_err(),
        "HTTP client should not be created in offline mode"
    );
}

#[test]
fn test_pkg_dirs_can_be_replaced() {
    let ctx = common::TestContextGuard::acquire();
    let first = tempdir().unwrap();
    let second = tempdir().unwrap();
    let filename = "archive.pkg.tar.zst";

    fs::write(first.path().join(filename), "first").unwrap();
    fs::write(second.path().join(filename), "second").unwrap();

    ctx.set_pkg_dirs(vec![first.path().to_path_buf()]);
    assert_eq!(
        pkgdir::find_in_pkg_dirs(filename).unwrap(),
        first.path().join(filename)
    );

    ctx.set_pkg_dirs(vec![second.path().to_path_buf()]);
    assert_eq!(
        pkgdir::find_in_pkg_dirs(filename).unwrap(),
        second.path().join(filename)
    );
}
