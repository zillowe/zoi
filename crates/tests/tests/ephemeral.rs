//! Integration tests for ephemeral environment management.
use std::fs;
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;
use zoi::cmd::shell;
use zoi::pkg::{config, db, local, plugin, resolve, types};
use zoi::utils;

mod common;

#[test]
fn test_ephemeral_environment_path() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    common::TestContextGuard::set_sysroot(root.clone());

    let home = root.join("home");
    fs::create_dir_all(&home).expect("unwrap failed");
    ctx.set_env_var("HOME", home.clone());

    let bin_name = "test-bin";
    let pkg_name = "test-pkg";
    let version = "1.0.0";
    let handle = "zoidberg";

    let cfg = types::Config {
        default_registry: Some(types::Registry {
            handle: handle.to_string(),
            url: "http://dummy".to_string(),
            advisory_prefix: None,
            authorities: None
        }),
        repos: vec!["core".to_string()],
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("unwrap failed");

    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{pkg_name}",
    repo = "core",
    version = "{version}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    bins = {{ "{bin_name}" }},
    types = {{ "pre-compiled" }}
}})

function verify()
    return true
end
"#
    );

    let db_root = resolve::get_db_root().expect("unwrap failed");
    let pkg_db_dir = db_root.join(handle).join("core").join(pkg_name);
    fs::create_dir_all(&pkg_db_dir).expect("unwrap failed");
    let pkg_lua_path = pkg_db_dir.join(format!("{pkg_name}.pkg.lua"));
    fs::write(&pkg_lua_path, &pkg_lua_content).expect("unwrap failed");

    let store_base =
        local::get_store_base_dir(types::Scope::User).expect("unwrap failed");
    let pkg_id = zoi::pkg::utils::generate_package_id(handle, "core", pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("unwrap failed");

    let binary_path = bin_dir.join(bin_name);
    #[cfg(unix)]
    {
        fs::write(&binary_path, "#!/bin/sh\necho 'hello'")
            .expect("unwrap failed");

        let mut perms = fs::metadata(&binary_path)
            .expect("unwrap failed")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms).expect("unwrap failed");
    }
    #[cfg(windows)]
    {
        fs::write(&binary_path, "@echo hello").expect("unwrap failed");
    }

    let manifest = types::InstallManifest {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        description: "test".to_string(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: Some(vec![bin_name.to_string()]),
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        dependencies_v2: None,
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        platform: zoi_core::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: vec![binary_path.to_string_lossy().to_string()],
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    };
    fs::write(
        version_dir.join("manifest.yaml"),
        serde_yaml::to_string(&manifest).expect("unwrap failed")
    )
    .expect("unwrap failed");

    let latest_path = pkg_path.join("latest");
    utils::symlink_file(&version_dir, &latest_path).expect("unwrap failed");

    let conn = db::open_connection(handle).expect("unwrap failed");
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
        None
    )
    .expect("unwrap failed");

    let pm = plugin::PluginManager::new().expect("unwrap failed");

    let run_cmd = format!("{} > {}", bin_name, root.join("out.txt").display());
    shell::enter_ephemeral_shell(
        &[pkg_name.to_string()],
        Some(run_cmd),
        false,
        Some(&pm)
    )
    .expect("unwrap failed");

    let out_file = root.join("out.txt");
    assert!(
        out_file.exists(),
        "Ephemeral command should have executed and created output file"
    );
    let content = fs::read_to_string(out_file).expect("unwrap failed");
    assert_eq!(content.trim(), "hello");
}
