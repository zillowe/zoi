//! Integration coverage for `zoi system distro build` marker creation.
use std::fs;

use tempfile::tempdir;
use zoi_system::distro::initialize_zoios_marker;

#[test]
fn test_initialize_zoios_marker() {
    let dir = tempdir().expect("temp dir");

    initialize_zoios_marker(dir.path(), Some("test-hostname"), false)
        .expect("write marker");

    let os_release = dir.path().join("etc/os-release");
    assert!(os_release.exists());

    let content = fs::read_to_string(os_release).expect("read marker");
    assert!(content.contains("ID=zoios"));
    assert!(content.contains("HOSTNAME=test-hostname"));
}
