use std::fs;
use tempfile::tempdir;
use zoi::pkg::{local, package, types};

mod common;

#[test]
fn test_atomic_shim_creation_rollback() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pkg_name = "shim-test";
    let version = "1.0.0";
    let handle = "local";
    let repo = "core";

    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_id = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    fs::write(bin_dir.join("bin1"), "echo 1").unwrap();
    fs::write(bin_dir.join("bin2"), "echo 2").unwrap();

    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{}",
    repo = "core",
    version = "{}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    bins = {{ "bin1", "bin2", "nonexistent" }}, -- 'nonexistent' will cause failure if we force it
    types = {{ "pre-compiled" }}
}})
"#,
        pkg_name, version
    );
    let pkg_lua_path = version_dir.join(format!("{}.pkg.lua", pkg_name));
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    let archive_path = root.join("dummy.pkg.tar.zst");
    fs::write(&archive_path, "").unwrap();

    let bin_root = root.join(".zoi/pkgs/bin");
    fs::create_dir_all(&bin_root).unwrap();

    let shim2_path = bin_root.join("bin2");
    fs::create_dir(&shim2_path).unwrap();

    let result = package::install::run(
        &archive_path,
        Some(types::Scope::User),
        handle,
        Some(version),
        true,
        None,
        true,
        None,
    );

    assert!(
        result.is_err(),
        "Install should have failed due to shim creation error"
    );

    let shim1_path = bin_root.join("bin1");
    assert!(!shim1_path.exists(), "Shim1 should have been rolled back");
}
