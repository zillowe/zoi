use std::fs;
use tempfile::tempdir;
use zoi::pkg::plugin::PluginManager;
use zoi::pkg::{db, local, shim, types};

mod common;

#[test]
fn test_shim_resolution_logic() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    ctx.set_env_var("HOME", home.clone());
    ctx.set_sysroot(root.clone());

    let bin_name = "hello";
    let pkg_name = "hello-pkg";
    let v1 = "1.0.0";
    let v2 = "2.0.0";

    let pkg_id = zoi::pkg::utils::generate_package_id("local", "core", pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();

    for v in &[v1, v2] {
        let pkg_path = store_base.join(&pkg_dir_name);
        let version_path = pkg_path.join(v);
        let bin_dir = version_path.join("bin");
        fs::create_dir_all(&bin_dir).expect("Failed to create store path");
        let binary_path = bin_dir.join(bin_name);
        fs::write(&binary_path, format!("echo 'Version {}'", v))
            .expect("Failed to write mock binary");
    }

    #[cfg(unix)]
    {
        let latest_path = store_base.join(&pkg_dir_name).join("latest");
        let _ = std::os::unix::fs::symlink(v1, latest_path);
    }

    let conn = db::open_connection("local").expect("Failed to open local db");
    let pkg = types::Package {
        name: pkg_name.to_string(),
        repo: "core".to_string(),
        version: Some(v1.to_string()),
        bins: Some(vec![bin_name.to_string()]),
        ..Default::default()
    };
    db::update_package(&conn, &pkg, "local", Some(types::Scope::User), None, None)
        .expect("Failed to update package in db");

    let pm = PluginManager::new().unwrap();

    let providers = db::find_provides("local", bin_name).unwrap();
    println!(
        "Providers for '{}': {:?}",
        bin_name,
        providers.iter().map(|(p, _)| &p.name).collect::<Vec<_>>()
    );
    assert!(!providers.is_empty(), "Should find providers in DB");

    let resolved = shim::resolve_to_installed_bin(bin_name, &pm).unwrap();
    println!("Resolved default: {}", resolved.display());
    assert!(resolved.to_string_lossy().contains(v1));

    ctx.set_env_var("ZOI_HELLO_VERSION", v2);
    let resolved_v2 = shim::resolve_to_installed_bin(bin_name, &pm).unwrap();
    println!("Resolved v2 override: {}", resolved_v2.display());
    assert!(resolved_v2.to_string_lossy().contains(v2));
}
