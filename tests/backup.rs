use std::fs;
use tempfile::tempdir;

#[test]
fn test_zoinew_upgrade_logic() {
    let dir = tempdir().unwrap();
    let old_ver_dir = dir.path().join("1.0.0");
    let new_ver_dir = dir.path().join("2.0.0");

    fs::create_dir_all(old_ver_dir.join("etc")).unwrap();
    fs::create_dir_all(new_ver_dir.join("etc")).unwrap();

    let config_rel_path = "etc/config.yaml";
    let old_config_path = old_ver_dir.join(config_rel_path);
    let new_config_path = new_ver_dir.join(config_rel_path);

    fs::write(&old_config_path, "user-custom-setting: true").unwrap();

    fs::write(&new_config_path, "user-custom-setting: false").unwrap();

    if old_config_path.exists() {
        if new_config_path.exists() {
            let extension = new_config_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let zoinew_path = new_config_path.with_extension(format!("{}.zoinew", extension));
            fs::rename(&new_config_path, &zoinew_path).unwrap();
        }
        fs::rename(&old_config_path, &new_config_path).unwrap();
    }

    assert_eq!(
        fs::read_to_string(&new_config_path).unwrap(),
        "user-custom-setting: true"
    );

    let zoinew_expected = new_ver_dir.join("etc/config.yaml.zoinew");
    assert!(zoinew_expected.exists());
    assert_eq!(
        fs::read_to_string(zoinew_expected).unwrap(),
        "user-custom-setting: false"
    );
}

#[test]
fn test_zoisave_uninstall_logic() {
    let dir = tempdir().unwrap();
    let package_dir = dir.path().join("my-pkg");
    let version_dir = package_dir.join("1.0.0");

    fs::create_dir_all(version_dir.join("etc")).unwrap();

    let config_rel_path = "etc/config.yaml";
    let config_path = version_dir.join(config_rel_path);

    fs::write(&config_path, "final-user-data").unwrap();

    let backup_dest = version_dir
        .parent()
        .unwrap()
        .join(format!("{}.zoisave", config_rel_path));

    if let Some(p) = backup_dest.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::rename(&config_path, &backup_dest).unwrap();

    assert!(!config_path.exists());
    assert!(backup_dest.exists());
    assert_eq!(fs::read_to_string(backup_dest).unwrap(), "final-user-data");
}
