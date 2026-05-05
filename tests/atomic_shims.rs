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

    // Create a mock package directory with some binaries
    let store_base = local::get_store_base_dir(types::Scope::User).unwrap();
    let pkg_id = zoi::pkg::utils::generate_package_id(handle, repo, pkg_name);
    let pkg_dir_name = zoi::pkg::utils::get_package_dir_name(&pkg_id, pkg_name);
    let pkg_path = store_base.join(&pkg_dir_name);
    let version_dir = pkg_path.join(version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    fs::write(bin_dir.join("bin1"), "echo 1").unwrap();
    fs::write(bin_dir.join("bin2"), "echo 2").unwrap();

    // Create a dummy .pkg.lua file
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

    // Mock archive path (not actually used since we mock the extraction)
    let archive_path = root.join("dummy.pkg.tar.zst");
    fs::write(&archive_path, "").unwrap();

    // We need to trigger the failure. In our implementation, if found_bin is false, it prints a warning but continues.
    // If create_shim fails, it rolls back.
    // To test rollback, we can make the bin_root directory read-only after creating one shim?
    // Or better, let's inject a failure in create_shim by making one of the targets non-writable.

    let bin_root = root.join(".zoi/pkgs/bin");
    fs::create_dir_all(&bin_root).unwrap();

    // Test that shims are NOT created if we return error early
    // Actually, let's test the case where create_shim returns Err.
    // We can't easily mock create_shim without changing code or using a mock trait.
    // But we can make a file at the target path that is a directory, which should make symlink_file fail on some systems or at least be something we can induce failure with.

    let shim2_path = bin_root.join("bin2");
    fs::create_dir(&shim2_path).unwrap(); // Making bin2 a directory should cause symlink_file to fail if it expects a file.

    // Run the install
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

    // It should fail because bin2 is a directory and cannot be replaced by a shim (file symlink)
    // and then it should rollback bin1.
    assert!(
        result.is_err(),
        "Install should have failed due to shim creation error"
    );

    let shim1_path = bin_root.join("bin1");
    assert!(!shim1_path.exists(), "Shim1 should have been rolled back");
}
