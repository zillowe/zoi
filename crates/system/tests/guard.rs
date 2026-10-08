//! Integration coverage for the `ZoiOS` environment guard.
use std::fs;

use tempfile::tempdir;
use zoi_core::utils::is_zoios;

#[test]
fn test_zoios_guard() {
    let dir = tempdir().expect("temp dir");
    let etc = dir.path().join("etc");
    fs::create_dir_all(&etc).expect("create etc");

    let os_release = etc.join("os-release");

    // Set sysroot for testing
    zoi_core::sysroot::set_sysroot(dir.path().to_path_buf());

    // Test non-ZoiOS
    fs::write(&os_release, "ID=ubuntu\nNAME=Ubuntu\n")
        .expect("write ubuntu marker");
    assert!(!is_zoios());

    // Test ZoiOS (ID=zoios)
    fs::write(&os_release, "ID=zoios\nNAME=ZoiOS\n")
        .expect("write zoios marker");
    assert!(is_zoios());

    // Test Parlex (ID=parlex)
    fs::write(&os_release, "ID=parlex\nNAME=Parlex Linux\n")
        .expect("write parlex marker");
    assert!(is_zoios());

    // Test ID_LIKE
    fs::write(&os_release, "ID=custom\nID_LIKE=zoios debian\n")
        .expect("write custom marker");
    assert!(is_zoios());
}
