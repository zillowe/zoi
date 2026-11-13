//! Integration coverage for `zoi system distro build` marker creation.
use std::fs;

use tempfile::tempdir;
use zoi_system::distro::{
    BOOTSTRAP_OS_RELEASE, finalize_zoios_marker, initialize_zoios_marker
};

#[test]
fn marker_is_a_valid_os_release_and_hostname_goes_to_etc_hostname() {
    let dir = tempdir().expect("temp dir");

    initialize_zoios_marker(dir.path(), Some("test-hostname"), false)
        .expect("write marker");
    zoi_core::sysroot::clear_sysroot();

    // The real /etc/os-release is left to the distribution's packages.
    assert!(!dir.path().join("etc/os-release").exists());

    let content = fs::read_to_string(dir.path().join(BOOTSTRAP_OS_RELEASE))
        .expect("read marker");
    assert_eq!(content, "ID=zoios\nNAME=ZoiOS\nPRETTY_NAME=\"ZoiOS\"\n");
    for line in content.lines() {
        let (key, _) = line.split_once('=').expect("KEY=value line");
        assert!(
            key.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            "malformed line: {line:?}"
        );
    }

    let hostname = fs::read_to_string(dir.path().join("etc/hostname"))
        .expect("read hostname");
    assert_eq!(hostname, "test-hostname\n");
}

#[test]
fn marker_is_dropped_when_a_package_provides_os_release() {
    let dir = tempdir().expect("temp dir");
    initialize_zoios_marker(dir.path(), None, false).expect("write marker");
    zoi_core::sysroot::clear_sysroot();

    fs::create_dir_all(dir.path().join("usr/lib")).expect("mkdir");
    fs::write(dir.path().join("usr/lib/os-release"), "ID=parlex\n")
        .expect("write");
    finalize_zoios_marker(dir.path(), false).expect("finalize");

    assert!(!dir.path().join(BOOTSTRAP_OS_RELEASE).exists());
    assert!(!dir.path().join("etc/os-release").exists());
}

#[test]
fn marker_becomes_os_release_when_no_package_provides_one() {
    let dir = tempdir().expect("temp dir");
    initialize_zoios_marker(dir.path(), None, false).expect("write marker");
    zoi_core::sysroot::clear_sysroot();

    finalize_zoios_marker(dir.path(), false).expect("finalize");

    assert!(!dir.path().join(BOOTSTRAP_OS_RELEASE).exists());
    let content =
        fs::read_to_string(dir.path().join("etc/os-release")).expect("read");
    assert!(content.starts_with("ID=zoios\n"));
}
