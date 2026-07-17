use std::fs;
use tar::Archive;
use tempfile::tempdir;
use zoi::pkg::package::build;
use zstd::stream::read::Decoder as ZstdDecoder;

mod common;

#[test]
fn test_fakeroot_build_ownership() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pkg_name = "fakeroot-test";
    let version = "1.0.0";
    let platform = zoi::utils::get_platform().unwrap();

    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{}",
    repo = "core",
    version = "{}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    types = {{ "source" }}
}})

function package()
    zmkdir("${{pkgstore}}/bin")
    cmd("echo 'echo hello' > test-bin")
    zcp("test-bin", "${{pkgstore}}/bin/test-bin")
end
"#,
        pkg_name, version
    );
    let pkg_lua_path = root.join(format!("{}.pkg.lua", pkg_name));
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    let output_dir = root.join("output");
    fs::create_dir(&output_dir).unwrap();

    build::run(
        &pkg_lua_path,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        Some(&output_dir),
        Some(version),
        None,
        false,
        "native",
        None,
        true,
        false,
    )
    .expect("Build should succeed");

    let archive_filename = format!("{}-{}-{}.zpa", pkg_name, version, platform);
    let archive_path = output_dir.join(archive_filename);
    assert!(archive_path.exists());

    let file = fs::File::open(archive_path).unwrap();
    let decoder = ZstdDecoder::new(file).unwrap();
    let mut archive = Archive::new(decoder);

    let mut found_bin = false;
    for entry in archive.entries().unwrap() {
        let entry = entry.unwrap();
        let header = entry.header();
        let path = entry.path().unwrap();

        if path.to_string_lossy().contains("bin/test-bin") {
            assert_eq!(header.uid().unwrap(), 0, "UID should be 0 (root)");
            assert_eq!(header.gid().unwrap(), 0, "GID should be 0 (root)");
            found_bin = true;
        }
    }

    assert!(found_bin, "Should have found test-bin in the archive");
}
