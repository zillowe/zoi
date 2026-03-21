use std::fs;
use tempfile::tempdir;
use zoi::cmd::shell;
use zoi::pkg::{config, db, local, plugin, resolve, sysroot, types};
use zoi::utils;

#[test]
fn test_ephemeral_environment_path() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    sysroot::set_sysroot(root.clone());

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    unsafe {
        std::env::set_var("HOME", home.clone());
    }

    let bin_name = "test-bin";
    let pkg_name = "test-pkg";
    let version = "1.0.0";
    let handle = "zoidberg";

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: handle.to_string(),
            url: "http://dummy".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        repos: vec!["core".to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{}",
    repo = "core",
    version = "{}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    bins = {{ "{}" }},
    types = {{ "pre-compiled" }}
}})

function verify()
    return true
end
"#,
        pkg_name, version, bin_name
    );

    let db_root = resolve::get_db_root().unwrap();
    let pkg_db_dir = db_root.join(handle).join("core").join(pkg_name);
    fs::create_dir_all(&pkg_db_dir).unwrap();
    let pkg_lua_path = pkg_db_dir.join(format!("{}.pkg.lua", pkg_name));
    fs::write(&pkg_lua_path, &pkg_lua_content).unwrap();

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_id = zoi::pkg::utils::generate_package_id(handle, "core", pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let binary_path = bin_dir.join(bin_name);
    #[cfg(unix)]
    {
        fs::write(&binary_path, "#!/bin/sh\necho 'hello'").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms).unwrap();
    }
    #[cfg(windows)]
    {
        fs::write(&binary_path, "@echo hello").unwrap();
    }

    let manifest = types::InstallManifest {
        name: pkg_name.to_string(),
        version: version.to_string(),
        sub_package: None,
        repo: "core".to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: Some(vec![bin_name.to_string()]),
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        service: None,
        installed_files: vec![binary_path.to_string_lossy().to_string()],
        installed_size: None,
    };
    fs::write(
        version_dir.join("manifest.yaml"),
        serde_yaml::to_string(&manifest).unwrap(),
    )
    .unwrap();

    let latest_path = pkg_path.join("latest");
    utils::symlink_dir(&version_dir, &latest_path).unwrap();

    let conn = db::open_connection(handle).unwrap();
    let pkg_meta = types::Package {
        name: pkg_name.to_string(),
        repo: "core".to_string(),
        version: Some(version.to_string()),
        bins: Some(vec![bin_name.to_string()]),
        ..Default::default()
    };
    db::update_package(
        &conn,
        &pkg_meta,
        handle,
        Some(types::Scope::User),
        None,
        None,
    )
    .unwrap();

    let pm = plugin::PluginManager::new().unwrap();

    let run_cmd = format!("{} > {}", bin_name, root.join("out.txt").display());
    shell::enter_ephemeral_shell(&[pkg_name.to_string()], Some(run_cmd), &pm).unwrap();

    let out_file = root.join("out.txt");
    assert!(
        out_file.exists(),
        "Ephemeral command should have executed and created output file"
    );
    let content = fs::read_to_string(out_file).unwrap();
    assert_eq!(content.trim(), "hello");
}
