//! Regression tests for archive trust handling during installation:
//! duplicate/nested manifest entries, the `.pkg.lua` digest bound into the
//! signed manifest, and delta rebuilds carrying the manifest verbatim.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use zoi_core::types::Scope;
use zoi_install::archive_trust;

mod common;

/// Writes a `.zpa`-style (tar + zstd) archive holding the given regular-file
/// entries, in order, without deduplicating names.
fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("create archive");
    let encoder = zstd::stream::write::Encoder::new(file, 0)
        .expect("zstd encoder")
        .auto_finish();
    let mut builder = tar::Builder::new(encoder);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *data)
            .expect("append entry");
    }
    builder.into_inner().expect("finish tar");
}

/// Re-packs an extracted archive directory into a `.zpa`.
fn repack_dir(dir: &Path, output: &Path) {
    let file = fs::File::create(output).expect("create archive");
    let encoder = zstd::stream::write::Encoder::new(file, 0)
        .expect("zstd encoder")
        .auto_finish();
    let mut builder = tar::Builder::new(encoder);
    builder.append_dir_all(".", dir).expect("append dir");
    builder.into_inner().expect("finish tar");
}

/// Extracts a `.zpa` into `dest`.
fn extract(archive: &Path, dest: &Path) {
    let file = fs::File::open(archive).expect("open archive");
    let decoder = zstd::stream::read::Decoder::new(file).expect("decoder");
    tar::Archive::new(decoder)
        .unpack(dest)
        .expect("unpack archive");
}

/// Builds a one-file package and returns the `.zpa` path.
fn build_package(
    root: &Path,
    name: &str,
    version: &str,
    content: &str,
    out: &str
) -> PathBuf {
    let platform = zoi_core::utils::get_platform().expect("platform");
    let pkg_lua_content = format!(
        r#"
metadata({{
    name = "{name}",
    repo = "core",
    version = "{version}",
    description = "test",
    maintainer = {{ name = "test", email = "test" }},
    types = {{ "source" }}
}})

function prepare()
    cmd("printf '{content}' > data.txt")
end

function package()
    zmkdir("${{pkgstore}}/share")
    zcp("data.txt", "${{pkgstore}}/share/data.txt")
end
"#
    );
    let src_dir = root.join(format!("src-{out}"));
    fs::create_dir_all(&src_dir).expect("src dir");
    let pkg_lua_path = src_dir.join(format!("{name}.pkg.lua"));
    fs::write(&pkg_lua_path, pkg_lua_content).expect("write pkg.lua");

    let output_dir = root.join(out);
    fs::create_dir_all(&output_dir).expect("output dir");
    zoi_package::build::run(
        &pkg_lua_path,
        Some("source"),
        std::slice::from_ref(&platform),
        None,
        zoi_core::types::SignMode::Embed,
        Some(&output_dir),
        Some(version),
        None,
        true,
        "native",
        None,
        false,
        false,
        false
    )
    .expect("build should succeed");
    output_dir.join(format!("{name}-{version}-{platform}.zpa"))
}

/// Installs `archive` into the user scope of the current test context.
fn install(archive: &Path, version: &str) -> anyhow::Result<Vec<String>> {
    zoi_install::pkg_install::run(
        archive,
        Some(Scope::User),
        "local",
        Some(version),
        true,
        None,
        false,
        None
    )
    .map(|(files, _)| files)
}

#[test]
fn duplicate_root_manifest_is_rejected() {
    let tmp = tempdir().expect("tempdir");
    let archive = tmp.path().join("dup.zpa");
    write_archive(
        &archive,
        &[
            ("manifest.json", b"{\"first\":true}".as_slice()),
            ("manifest.json", b"{\"second\":true}".as_slice())
        ]
    );
    assert!(archive_trust::scan_archive(&archive).is_err());
}

#[test]
fn duplicate_root_signature_is_rejected() {
    let tmp = tempdir().expect("tempdir");
    let archive = tmp.path().join("dup-sig.zpa");
    write_archive(
        &archive,
        &[
            ("manifest.json", b"{}".as_slice()),
            ("manifest.sig", b"one".as_slice()),
            ("./manifest.sig", b"two".as_slice())
        ]
    );
    assert!(archive_trust::scan_archive(&archive).is_err());
}

#[test]
fn nested_manifest_next_to_root_manifest_is_rejected() {
    let tmp = tempdir().expect("tempdir");
    let archive = tmp.path().join("nested.zpa");
    write_archive(
        &archive,
        &[
            ("manifest.json", b"{}".as_slice()),
            ("pool/manifest.json", b"{\"evil\":true}".as_slice())
        ]
    );
    assert!(archive_trust::scan_archive(&archive).is_err());
}

#[test]
fn nested_manifest_in_legacy_payload_is_ignored() {
    // Legacy `data/` archives have no root manifest; a `manifest.json` that
    // is part of the payload must not make them uninstallable.
    let tmp = tempdir().expect("tempdir");
    let archive = tmp.path().join("legacy.zpa");
    write_archive(
        &archive,
        &[
            ("legacy.pkg.lua", b"-- definition".as_slice()),
            ("data/pkgstore/manifest.json", b"{\"app\":true}".as_slice())
        ]
    );
    let entries =
        archive_trust::scan_archive(&archive).expect("legacy scan succeeds");
    assert!(entries.manifest.is_none());
    assert_eq!(
        entries
            .single_pkg_lua()
            .expect("single definition")
            .map(|(name, _)| name),
        Some("legacy.pkg.lua")
    );
}

#[test]
fn classify_entry_normalizes_like_extraction() {
    use archive_trust::{TrustEntryKind, classify_entry};
    assert_eq!(
        classify_entry(Path::new("./manifest.json")),
        Some((TrustEntryKind::Manifest, Some("manifest.json".to_string())))
    );
    assert_eq!(
        classify_entry(Path::new("/manifest.sig")),
        Some((
            TrustEntryKind::ManifestSig,
            Some("manifest.sig".to_string())
        ))
    );
    assert_eq!(
        classify_entry(Path::new("x/manifest.json")),
        Some((TrustEntryKind::Manifest, None))
    );
    assert_eq!(
        classify_entry(Path::new("foo.pkg.lua.sig")),
        Some((
            TrustEntryKind::PkgLuaSig,
            Some("foo.pkg.lua.sig".to_string())
        ))
    );
    assert_eq!(classify_entry(Path::new("../manifest.json")), None);
    assert_eq!(classify_entry(Path::new("pool/sha256-abc")), None);
}

#[test]
fn install_refuses_archive_with_duplicate_manifest() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let archive = root.join("dup-install.zpa");
    write_archive(
        &archive,
        &[
            ("dup.pkg.lua", b"-- never parsed".as_slice()),
            ("manifest.json", b"{}".as_slice()),
            ("manifest.json", b"{}".as_slice())
        ]
    );
    assert!(install(&archive, "1.0.0").is_err());
}

#[test]
fn built_manifest_binds_the_bundled_pkg_lua() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let archive = build_package(&root, "bind-test", "1.0.0", "hello", "out");
    let entries = archive_trust::scan_archive(&archive).expect("scan");
    let manifest = entries.manifest.as_deref().expect("root manifest");
    let (_, pkg_lua) = entries
        .single_pkg_lua()
        .expect("single definition")
        .expect("definition present");

    let recorded = archive_trust::recorded_pkg_lua_hash(manifest)
        .expect("manifest records the definition digest");
    assert_eq!(
        recorded,
        format!(
            "sha256-{}",
            archive_trust::sha256_hex(pkg_lua).expect("hash")
        )
    );
    assert!(
        archive_trust::check_pkg_lua_binding(manifest, Some(pkg_lua))
            .expect("binding holds")
    );
    assert!(
        archive_trust::check_pkg_lua_binding(
            manifest,
            Some(b"tampered".as_slice())
        )
        .is_err()
    );
}

#[test]
fn install_rejects_swapped_pkg_lua() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let archive = build_package(&root, "swap-test", "1.0.0", "hello", "out");

    // Repacking the untouched contents must still install, so a failure
    // below is attributable to the swapped definition alone.
    let pristine_dir = root.join("pristine");
    fs::create_dir_all(&pristine_dir).expect("dir");
    extract(&archive, &pristine_dir);
    let pristine = root.join("pristine.zpa");
    repack_dir(&pristine_dir, &pristine);
    install(&pristine, "1.0.0").expect("repacked archive installs");

    let tampered_dir = root.join("tampered");
    fs::create_dir_all(&tampered_dir).expect("dir");
    extract(&archive, &tampered_dir);
    let pkg_lua = tampered_dir.join("swap-test.pkg.lua");
    let mut content = fs::read_to_string(&pkg_lua).expect("read pkg.lua");
    content.push_str("\n-- injected\n");
    fs::write(&pkg_lua, content).expect("write pkg.lua");
    let tampered = root.join("tampered.zpa");
    repack_dir(&tampered_dir, &tampered);

    let error = install(&tampered, "1.0.0")
        .expect_err("a definition not matching the manifest digest");
    assert!(
        error.to_string().contains("does not match"),
        "unexpected error: {error}"
    );
}

#[test]
fn delta_rebuild_keeps_manifest_bytes_verbatim() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", root.clone());
    common::TestContextGuard::set_sysroot(root.clone());

    let v1 = build_package(&root, "delta-raw", "1.0.0", "content-one", "o1");
    let v2 = build_package(&root, "delta-raw", "1.1.0", "content-two", "o2");

    let delta = root.join("delta-raw.zdelta");
    zoi_package::delta::create_zpa_delta(&v1, &v2, &delta, None)
        .expect("create delta");
    let rebuilt = root.join("rebuilt.zpa");
    zoi_package::delta::apply_zpa_delta(&v1, &delta, &rebuilt)
        .expect("apply delta");

    let original = archive_trust::scan_archive(&v2).expect("scan v2");
    let rebuilt_entries =
        archive_trust::scan_archive(&rebuilt).expect("scan rebuilt");
    // Byte-identical manifests are what keep an embedded signature valid
    // (and keep the `.pkg.lua` digest) across a delta upgrade.
    assert_eq!(original.manifest, rebuilt_entries.manifest);
    let manifest = rebuilt_entries.manifest.as_deref().expect("manifest");
    let pkg_lua = rebuilt_entries
        .single_pkg_lua()
        .expect("single definition")
        .map(|(_, bytes)| bytes);
    assert!(
        archive_trust::check_pkg_lua_binding(manifest, pkg_lua)
            .expect("binding survives the delta")
    );
}
