use std::fs;
use tempfile::tempdir;
use zoi_core::utils::is_zoios;

#[test]
fn test_zoios_guard() {
    let dir = tempdir().unwrap();
    let etc = dir.path().join("etc");
    fs::create_dir_all(&etc).unwrap();

    let os_release = etc.join("os-release");

    // Set sysroot for testing
    zoi_core::sysroot::set_sysroot(dir.path().to_path_buf());

    // Test non-ZoiOS
    fs::write(&os_release, "ID=ubuntu\nNAME=Ubuntu\n").unwrap();
    assert!(!is_zoios());

    // Test ZoiOS (ID=zoios)
    fs::write(&os_release, "ID=zoios\nNAME=ZoiOS\n").unwrap();
    assert!(is_zoios());

    // Test Parlex (ID=parlex)
    fs::write(&os_release, "ID=parlex\nNAME=Parlex Linux\n").unwrap();
    assert!(is_zoios());

    // Test ID_LIKE
    fs::write(&os_release, "ID=custom\nID_LIKE=zoios debian\n").unwrap();
    assert!(is_zoios());
}
