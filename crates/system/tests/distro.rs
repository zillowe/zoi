use std::fs;
use tempfile::tempdir;
use zoi_system::distro::initialize_zoios_marker;

#[test]
fn test_initialize_zoios_marker() {
    let dir = tempdir().unwrap();

    initialize_zoios_marker(dir.path(), Some("test-hostname"), false).unwrap();

    let os_release = dir.path().join("etc/os-release");
    assert!(os_release.exists());

    let content = fs::read_to_string(os_release).unwrap();
    assert!(content.contains("ID=zoios"));
    assert!(content.contains("HOSTNAME=test-hostname"));
}
