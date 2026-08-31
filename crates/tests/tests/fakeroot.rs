//! Integration tests for fakeroot-based package building.

use std::fs;
use std::io::Read;

use tar::Archive;
use tempfile::tempdir;
use zoi::pkg::package::build;
use zoi_core::types::PooledZpaManifest;
use zstd::stream::read::Decoder as ZstdDecoder;

mod common;

#[test]
fn test_fakeroot_build_ownership() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let pkg_name = "fakeroot-test";
    let version = "1.0.0";
    let platform = zoi::utils::get_platform().expect("unwrap failed");

    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{pkg_name}",
    repo = "core",
    version = "{version}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    types = {{ "source" }}
}})

function package()
    zmkdir("${{pkgstore}}/bin")
    cmd("echo 'echo hello' > test-bin")
    zcp("test-bin", "${{pkgstore}}/bin/test-bin")
end
"#
    );
    let pkg_lua_path = root.join(format!("{pkg_name}.pkg.lua"));
    fs::write(&pkg_lua_path, pkg_lua_content).expect("unwrap failed");

    let output_dir = root.join("output");
    fs::create_dir(&output_dir).expect("unwrap failed");

    build::run(
        &pkg_lua_path,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        zoi_core::types::SignMode::Embed,
        Some(&output_dir),
        Some(version),
        None,
        false,
        "native",
        None,
        true,
        false,
        false
    )
    .expect("Build should succeed");

    let archive_filename = format!("{pkg_name}-{version}-{platform}.zpa");
    let archive_path = output_dir.join(archive_filename);
    assert!(archive_path.exists());

    // Read the pooled manifest to verify the binary is mapped to the pool as
    // root-owned.
    let file = fs::File::open(&archive_path).expect("unwrap failed");
    let decoder = ZstdDecoder::new(file).expect("unwrap failed");
    let mut archive = Archive::new(decoder);
    let mut manifest = None;
    for entry in archive.entries().expect("unwrap failed") {
        let mut entry = entry.expect("unwrap failed");
        if entry
            .path()
            .expect("unwrap failed")
            .to_string_lossy()
            .ends_with("manifest.json")
        {
            let mut json = String::new();
            entry.read_to_string(&mut json).expect("unwrap failed");
            manifest = Some(
                serde_json::from_str::<PooledZpaManifest>(&json)
                    .expect("manifest should be a valid pooled manifest")
            );
        }
    }
    let manifest = manifest.expect("manifest.json should exist in the archive");
    assert!(!manifest.pool.is_empty(), "pool should not be empty");

    let scope_mapping = manifest
        .mappings
        .get("")
        .and_then(|m| m.scopes.get(&zoi_core::types::Scope::User))
        .expect("main package should have a User scope mapping");
    let mapped = scope_mapping
        .files
        .iter()
        .find(|f| f.dest.ends_with("bin/test-bin"))
        .expect("bin/test-bin should be in the manifest");
    assert_eq!(
        mapped.owner.as_deref(),
        Some("root"),
        "manifest should record the binary as owned by root"
    );
    assert_eq!(
        mapped.group.as_deref(),
        Some("root"),
        "manifest should record the binary as grouped by root"
    );

    // Stream the pooled file (whose name is the manifest hash key) out of the
    // archive and verify its on-disk tar owner matches root (UID/GID 0).
    let file = fs::File::open(&archive_path).expect("unwrap failed");
    let decoder = ZstdDecoder::new(file).expect("unwrap failed");
    let mut archive = Archive::new(decoder);

    let mut found_bin = false;
    for entry in archive.entries().expect("unwrap failed") {
        let entry = entry.expect("unwrap failed");
        let path = entry.path().expect("unwrap failed");
        let path_str = path.to_string_lossy();

        // Pool files are named after their manifest hash key
        // ("sha256-<hex>").
        if path_str.ends_with(&format!("/{}", mapped.hash)) {
            assert_eq!(
                entry.header().uid().expect("unwrap failed"),
                0,
                "UID should be 0 (root)"
            );
            assert_eq!(
                entry.header().gid().expect("unwrap failed"),
                0,
                "GID should be 0 (root)"
            );
            found_bin = true;
        }
    }

    assert!(
        found_bin,
        "Should have found the pooled binary in the archive"
    );
}
