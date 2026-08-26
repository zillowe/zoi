//! Integration tests for package relocation and path adjustment.

use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use tar::Archive;
use tempfile::tempdir;
use zoi::pkg::package::{build, relocate};
use zoi::utils::get_platform;
use zoi_core::types::PooledZpaManifest;
use zstd::stream::read::Decoder as ZstdDecoder;

mod common;

#[test]
fn test_relocation_engine_identifies_elf_files() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let staging = tmp.path().to_path_buf();

    let pkgstore = staging.join("data/pkgstore");
    let bin_dir = pkgstore.join("bin");
    fs::create_dir_all(&bin_dir).expect("unwrap failed");

    let elf_path = bin_dir.join("my-bin");
    // Write ELF magic number
    fs::write(&elf_path, b"\x7fELF some other content").expect("unwrap failed");

    let non_elf_path = bin_dir.join("README.txt");
    fs::write(&non_elf_path, b"Just text").expect("unwrap failed");

    // relocate_elfs will fail when trying to parse the mock ELF with arwen,
    // but it should at least try to relocate it and log a warning.
    // We can't easily assert on stdout/stderr here without more infra,
    // but we can ensure it doesn't crash.
    let result = relocate::relocate_elfs(&staging, true);
    assert!(result.is_ok());
}

#[test]
fn test_relocation_engine_complex_depth() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let staging = tmp.path().to_path_buf();

    let pkgstore = staging.join("data/pkgstore");
    let plugin_dir = pkgstore.join("lib/plugins/extra");
    fs::create_dir_all(&plugin_dir).expect("unwrap failed");

    let plugin_path = plugin_dir.join("my-plugin.so");
    fs::write(&plugin_path, b"\x7fELF plugin content").expect("unwrap failed");

    // The relocation engine should identify this and try to apply RPATHs.
    // Since it's depth 3 from pkgstore (lib/plugins/extra/), it should include
    // $ORIGIN/../../..
    let result = relocate::relocate_elfs(&staging, true);
    assert!(result.is_ok());
}

// Regression test: ELF relocation must run before pooling. If it runs on the
// pool directory after `pool_files` has recorded sizes and hashes, the pool
// files contain relocated bytes while the manifest still describes the
// pre-relocation content, causing installation to fail with
// "Pool file size does not match manifest".
#[test]
fn test_pooled_manifest_matches_pool_contents_after_relocation() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let pkg_name = "relocatable-test";
    let version = "1.0.0";
    let platform = get_platform().expect("unwrap failed");

    // Stage a genuine ELF binary (this test executable) next to the package
    // definition so the build actually exercises the relocation engine.
    let self_exe = std::env::current_exe().expect("unwrap failed");
    fs::copy(&self_exe, root.join("reloc-test-bin")).expect("unwrap failed");

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
    zcp("reloc-test-bin", "${{pkgstore}}/bin/reloc-test-bin")
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
        Some(&output_dir),
        Some(version),
        None,
        true,
        "native",
        None,
        true,
        false,
        false
    )
    .expect("Build should succeed");

    let archive_path =
        output_dir.join(format!("{pkg_name}-{version}-{platform}.zpa"));
    assert!(archive_path.exists());

    let read_manifest = || -> PooledZpaManifest {
        let file = fs::File::open(&archive_path).expect("unwrap failed");
        let decoder = ZstdDecoder::new(file).expect("unwrap failed");
        let mut archive = Archive::new(decoder);
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
                return serde_json::from_str(&json)
                    .expect("manifest should be a valid pooled manifest");
            }
        }
        panic!("manifest.json should exist in the archive");
    };
    let manifest = read_manifest();
    assert!(!manifest.pool.is_empty(), "pool should not be empty");

    // Stream every pool file out of the archive and verify it matches the
    // size and hash the manifest recorded for it.
    let file = fs::File::open(&archive_path).expect("unwrap failed");
    let decoder = ZstdDecoder::new(file).expect("unwrap failed");
    let mut archive = Archive::new(decoder);

    let mut checked = 0usize;
    for entry in archive.entries().expect("unwrap failed") {
        let mut entry = entry.expect("unwrap failed");
        let entry_path = entry.path().expect("unwrap failed").to_path_buf();
        // Archive entry paths may or may not carry a leading "./"
        if !entry_path.to_string_lossy().contains("/pool/")
            && entry_path.parent().is_none_or(|p| p != Path::new("pool"))
        {
            continue;
        }
        let file_name = entry_path
            .file_name()
            .expect("unwrap failed")
            .to_string_lossy()
            .to_string();

        // Pool files are named after their manifest key ("sha256-<hex>")
        let Some(expected_hash) = file_name.strip_prefix("sha256-") else {
            continue;
        };
        let Some(pool_entry) = manifest.pool.get(&file_name) else {
            continue;
        };

        let mut hasher = Sha256::new();
        let mut len: u64 = 0;
        let mut buf = [0u8; 8192];
        loop {
            let n = entry.read(&mut buf).expect("unwrap failed");
            if n == 0 {
                break;
            }
            if let Some(chunk) = buf.get(..n) {
                hasher.update(chunk);
            }
            len += n as u64;
        }

        assert_eq!(
            len, pool_entry.size,
            "pool size must match manifest for {file_name}"
        );
        assert_eq!(
            hex::encode(hasher.finalize()),
            expected_hash,
            "pool content must match manifest hash for {file_name}"
        );
        checked += 1;
    }
    assert!(checked > 0, "should have verified at least one pooled file");
}
