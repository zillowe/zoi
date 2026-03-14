use std::fs;
use tempfile::tempdir;
use zoi::pkg::{local, service, sysroot, types};

#[test]
fn test_linux_service_lifecycle() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();

    unsafe {
        std::env::set_var("HOME", home.clone());
    }

    sysroot::set_sysroot(root.clone());

    let pkg_name = "test-service";
    let version = "1.0.0";

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_id = zoi::pkg::utils::generate_package_id("local", "core", pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);

    let pkg_path = store_base.join(&pkg_dir_name);
    let version_path = pkg_path.join(version);
    fs::create_dir_all(&version_path).expect("Failed to create version path");

    let manifest = types::InstallManifest {
        name: pkg_name.to_string(),
        version: version.to_string(),
        sub_package: None,
        repo: "core".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: None,
        service: Some(types::Service {
            run: "/usr/bin/test-pkg".to_string(),
            working_dir: Some("/tmp".to_string()),
            env: Some(
                [("ZOI_DEBUG".to_string(), "1".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
            ),
            log_path: Some("/tmp/test.log".to_string()),
            error_log_path: Some("/tmp/test-err.log".to_string()),
            run_at_load: true,
        }),
        installed_files: vec![],
        installed_size: None,
    };

    let manifest_path = version_path.join("manifest.yaml");
    fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap())
        .expect("Failed to write manifest");

    #[cfg(unix)]
    {
        let latest_path = pkg_path.join("latest");
        std::os::unix::fs::symlink(version, latest_path).unwrap();
    }

    let _ = service::manage_service(pkg_name, service::ServiceAction::Start);

    #[cfg(target_os = "linux")]
    {
        let unit_path = home
            .join(".config/systemd/user")
            .join(format!("zoi-{}.service", pkg_name));
        assert!(unit_path.exists(), "Unit file should be created");

        service::cleanup_service(pkg_name, types::Scope::User).unwrap();
        assert!(
            !unit_path.exists(),
            "Unit file should be removed after cleanup"
        );
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = home
            .join("Library/LaunchAgents")
            .join(format!("zoi-{}.plist", pkg_name));
        assert!(plist_path.exists());
        service::cleanup_service(pkg_name, types::Scope::User).unwrap();
        assert!(!plist_path.exists());
    }
}
