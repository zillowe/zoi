//! Integration tests for the zstd-compressed registry database (`.zrepo`)
//! transport and its incremental `.zdelta` updates.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::json;
use tempfile::tempdir;
use zbsdiff::zdelta;
use zoi_core::types::RepoConfig;
use zoi_sync::zrepo;

mod common;

/// Builds the uncompressed tar of a registry snapshot.
///
/// The tree always carries a `repo.yaml` and a `packages.json` so it mirrors
/// what a real published registry looks like, plus one package definition whose
/// body the tests vary between revisions. `filler` adds that many unchanged
/// package definitions, standing in for the bulk of a real registry.
fn build_registry_tar_with_filler(
    pkg_lua_body: &str,
    filler: usize
) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());

    let append =
        |builder: &mut tar::Builder<Vec<u8>>, name: &str, body: &str| {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, name, Cursor::new(body.as_bytes()))
                .expect("tar append should succeed");
        };

    append(
        &mut builder,
        "repo.yaml",
        "version: \"2\"\n\
         name: testreg\n\
         description: A test registry\n\
         git: []\n\
         repos: []\n\
         zrepo:\n\
         \x20 - type: main\n\
         \x20   url: https://example.com/testreg.zrepo\n",
    );
    append(
        &mut builder,
        "packages.json",
        r#"{"version":"2","packages":{}}"#
    );

    for index in 0..filler {
        let name = format!("main/tool-{index:04}/tool-{index:04}.pkg.lua");
        let body = format!(
            "metadata({{name=\"tool-{index:04}\",version=\"1.{index}.0\",\
             description=\"Filler package {index} with a body long enough to \
             compress and diff like a real package definition would.\"}})\n"
        );
        append(&mut builder, &name, &body);
    }

    append(&mut builder, "main/ripgrep/ripgrep.pkg.lua", pkg_lua_body);

    builder.into_inner().expect("tar build should succeed")
}

/// Builds a minimal registry snapshot.
fn build_registry_tar(pkg_lua_body: &str) -> Vec<u8> {
    build_registry_tar_with_filler(pkg_lua_body, 0)
}

/// Writes `tar_bytes` as a zstd-compressed `.zrepo` file.
fn write_zrepo(path: &Path, tar_bytes: &[u8]) {
    let compressed = zdelta::zstd_compress(tar_bytes)
        .expect("zstd compression should succeed");
    fs::write(path, compressed).expect("zrepo write should succeed");
}

/// Writes a `.zdelta` container carrying a bsdiff patch from `old_tar` to
/// `new_tar`, with the given `meta` overrides merged over the required fields.
fn write_zrepo_delta(
    path: &Path,
    old_tar: &[u8],
    new_tar: &[u8],
    meta_overrides: &serde_json::Value
) {
    let diff =
        zdelta::diff_bytes(old_tar, new_tar).expect("diff should succeed");

    let mut meta = json!({
        "format": zdelta::ZDELTA_FORMAT_ID,
        "type": "zrepo",
        "base_sha256": sha256_hex(old_tar),
        "target_sha256": sha256_hex(new_tar),
    });
    // Let a test override any field, e.g. to forge a hash.
    if let (Some(base), Some(overrides)) =
        (meta.as_object_mut(), meta_overrides.as_object())
    {
        for (key, value) in overrides {
            base.insert(key.clone(), value.clone());
        }
    }

    zdelta::write_container(&meta, &[("data.bsdiff".to_string(), diff)], path)
        .expect("zdelta write should succeed");
}

/// SHA-256 hex digest of `data`.
fn sha256_hex(data: &[u8]) -> String {
    zoi_core::hash::calculate_reader_hash(
        &mut Cursor::new(data),
        zoi_core::hash::HashAlgorithm::Sha256
    )
    .expect("hashing should succeed")
    .1
}

/// Decompresses a `.zrepo` file back to its tar bytes.
fn read_zrepo(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).expect("zrepo read should succeed");
    zdelta::zstd_decompress(&bytes).expect("zstd decompression should succeed")
}

/// Reads a single file out of a tar archive's bytes.
fn read_tar_entry(tar_bytes: &[u8], wanted: &str) -> String {
    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    for entry in archive.entries().expect("tar entries") {
        let mut entry = entry.expect("tar entry");
        let name = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .to_string();
        if name == wanted {
            let mut body = String::new();
            std::io::Read::read_to_string(&mut entry, &mut body)
                .expect("read entry");
            return body;
        }
    }
    panic!("'{wanted}' not found in archive");
}

const REVISION_ONE: &str = "metadata({name=\"ripgrep\",version=\"14.1.0\"})";
const REVISION_TWO: &str = "metadata({name=\"ripgrep\",version=\"14.1.1\"})";

#[test]
fn zrepo_delta_roundtrip_rebuilds_the_new_snapshot() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);
    let new_tar = build_registry_tar(REVISION_TWO);

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("testreg.zrepo.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);
    write_zrepo_delta(&patch, &old_tar, &new_tar, &json!({}));

    zrepo::apply_delta(&base, &patch, &out).expect("delta should apply");

    // The rebuilt snapshot must be byte-identical to a full download, so the
    // incremental path is invisible to every consumer of the registry.
    let rebuilt = read_zrepo(&out);
    assert_eq!(rebuilt, new_tar, "rebuilt tar must equal the published tar");
    assert_eq!(
        read_tar_entry(&rebuilt, "main/ripgrep/ripgrep.pkg.lua"),
        REVISION_TWO
    );
}

#[test]
fn zrepo_delta_is_smaller_than_a_full_download() {
    let tmp = tempdir().expect("tempdir");
    // A realistic registry: many packages, of which one changed. This is the
    // case the incremental update exists for, and the reason the patch is
    // computed over the uncompressed tar rather than the compressed snapshot -
    // unchanged tar blocks are byte-identical and diff to almost nothing.
    let old_tar = build_registry_tar_with_filler(REVISION_ONE, 400);
    let new_tar = build_registry_tar_with_filler(REVISION_TWO, 400);

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("testreg.zrepo.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);
    write_zrepo_delta(&patch, &old_tar, &new_tar, &json!({}));

    zrepo::apply_delta(&base, &patch, &out).expect("delta should apply");

    let patch_size = fs::metadata(&patch).expect("patch size").len();
    let full_size = fs::metadata(&base).expect("base size").len();
    assert!(
        patch_size * 10 < full_size,
        "an incremental update ({patch_size} bytes) must be far smaller than \
         a full download ({full_size} bytes)"
    );
}

#[test]
fn zrepo_delta_rejects_a_base_the_patch_was_not_built_against() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);
    let new_tar = build_registry_tar(REVISION_TWO);
    // A different base than the one the patch targets, as happens when a
    // client missed a sync.
    let other_tar =
        build_registry_tar("metadata({name=\"ripgrep\",version=\"14.0.9\"})");

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("testreg.zrepo.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &other_tar);
    write_zrepo_delta(&patch, &old_tar, &new_tar, &json!({}));

    let err = zrepo::apply_delta(&base, &patch, &out)
        .expect_err("a mismatched base must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("does not match the base"),
        "unexpected error: {message}"
    );
    assert!(
        !out.exists(),
        "a rejected delta must not leave a snapshot behind"
    );
}

#[test]
fn zrepo_delta_rejects_a_rebuilt_snapshot_with_the_wrong_hash() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);
    let new_tar = build_registry_tar(REVISION_TWO);

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("testreg.zrepo.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);
    // Forge the target hash so the rebuilt snapshot cannot be trusted.
    write_zrepo_delta(
        &patch,
        &old_tar,
        &new_tar,
        &json!({ "target_sha256": sha256_hex(b"something else entirely") })
    );

    let err = zrepo::apply_delta(&base, &patch, &out)
        .expect_err("a hash mismatch must be rejected");
    assert!(
        err.to_string().contains("does not match the expected hash"),
        "unexpected error: {err}"
    );
}

#[test]
fn zrepo_delta_rejects_a_patch_meant_for_another_target() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);
    let new_tar = build_registry_tar(REVISION_TWO);

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("testreg.zrepo.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);
    // A `.zpa` update patch aimed at a different artifact type must not be
    // applied to a registry snapshot.
    write_zrepo_delta(&patch, &old_tar, &new_tar, &json!({ "type": "zpa" }));

    let err = zrepo::apply_delta(&base, &patch, &out)
        .expect_err("a foreign patch type must be rejected");
    assert!(
        err.to_string().contains("expected 'zrepo'"),
        "unexpected error: {err}"
    );
}

#[test]
fn zrepo_delta_accepts_a_bare_bsdiff_patch() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);
    let new_tar = build_registry_tar(REVISION_TWO);

    let base = tmp.path().join("testreg.zrepo");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);

    // The single-target shape: just a zstd-compressed raw bsdiff patch, with
    // no container metadata at all.
    let patch_bytes =
        zdelta::diff_bytes(&old_tar, &new_tar).expect("diff should succeed");
    let patch = tmp.path().join("bare.zdelta");
    fs::write(
        &patch,
        zdelta::zstd_compress(&patch_bytes).expect("compress patch")
    )
    .expect("write patch");

    zrepo::apply_delta(&base, &patch, &out).expect("bare patch should apply");
    assert_eq!(read_zrepo(&out), new_tar);
}

#[test]
fn zrepo_delta_rejects_garbage() {
    let tmp = tempdir().expect("tempdir");
    let old_tar = build_registry_tar(REVISION_ONE);

    let base = tmp.path().join("testreg.zrepo");
    let patch = tmp.path().join("garbage.zdelta");
    let out = tmp.path().join("rebuilt.zrepo");

    write_zrepo(&base, &old_tar);
    fs::write(
        &patch,
        zdelta::zstd_compress(b"neither a container nor a bsdiff patch")
            .expect("compress")
    )
    .expect("write patch");

    zrepo::apply_delta(&base, &patch, &out)
        .expect_err("garbage must be rejected");
}

/// Parses a `repo.yaml` declaring `zrepo` entries in the given order.
fn repo_config_with_zrepo_links(yaml: &str) -> RepoConfig {
    serde_yaml::from_str(yaml).expect("repo.yaml should parse")
}

#[test]
fn zrepo_candidates_prefer_main_then_fall_back_to_mirrors() {
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: mirror
    url: https://eu.example.com/testreg.zrepo
  - type: mirror
    url: https://us.example.com/testreg.zrepo
  - type: main
    url: https://main.example.com/testreg.zrepo
"#
    );

    let candidates = zrepo::candidate_links(&repo_config);
    let urls: Vec<&str> =
        candidates.iter().map(|link| link.url.as_str()).collect();

    assert_eq!(
        urls,
        vec![
            "https://main.example.com/testreg.zrepo",
            "https://eu.example.com/testreg.zrepo",
            "https://us.example.com/testreg.zrepo",
        ],
        "the main tier must be tried first, mirrors only as a fallback"
    );
}

#[test]
fn zrepo_candidates_keep_a_registry_without_a_main_tier_usable() {
    // A registry that labels every entry `mirror` still syncs; the entries are
    // simply tried in the order they were declared.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: mirror
    url: https://a.example.com/testreg.zrepo
  - type: mirror
    url: https://b.example.com/testreg.zrepo
"#
    );

    let urls: Vec<String> = zrepo::candidate_links(&repo_config)
        .iter()
        .map(|link| link.url.clone())
        .collect();

    assert_eq!(
        urls,
        vec![
            "https://a.example.com/testreg.zrepo",
            "https://b.example.com/testreg.zrepo"
        ]
    );
}

#[test]
fn zrepo_delta_url_defaults_to_the_conventional_suffix() {
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: testreg
description: A test registry
git: []
repos: []
zrepo:
  - type: main
    url: https://example.com/testreg.zrepo
  - type: mirror
    url: https://mirror.example.com/testreg.zrepo
    delta: https://patch.example.com/testreg.delta
"#
    );

    let candidates = zrepo::candidate_links(&repo_config);
    let main = candidates.first().expect("main link should be present");
    let mirror = candidates.get(1).expect("mirror link should be present");

    assert_eq!(
        zrepo::delta_url(main),
        "https://example.com/testreg.zrepo.zdelta",
        "an entry without an explicit delta resolves to <url>.zdelta"
    );
    assert_eq!(
        zrepo::delta_url(mirror),
        "https://patch.example.com/testreg.delta",
        "an explicit delta URL must win"
    );
}

#[test]
fn zrepo_base_snapshot_lives_next_to_the_registry_index() {
    let db_root = PathBuf::from("/tmp/zoi-db");
    assert_eq!(
        zrepo::base_artifact_path(&db_root, "testreg"),
        db_root.join("testreg.zrepo")
    );
}

#[test]
fn git_only_repo_yaml_declares_no_zrepo_links() {
    // The existing registry format must keep working: a `repo.yaml` with only
    // `git:` links yields no `.zrepo` candidates, so sync falls through to the
    // Git transport.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: zoidberg
description: Official Zoi packages repository
git:
  - type: official
    url: https://gitlab.com/zillowe/zillwen/zusty/zoidberg
repos: []
"#
    );

    assert!(
        zrepo::candidate_links(&repo_config).is_empty(),
        "a git-only repo.yaml must not select the snapshot transport"
    );
}

#[test]
fn a_snapshot_only_registry_needs_no_git_section() {
    // A registry published as a snapshot instead of a Git repository has
    // nothing to put under `git:`, so an absent section must parse rather than
    // failing the whole manifest.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: snapshotonly
description: A registry with no Git repository
zrepo:
  - type: main
    url: https://example.com/snapshotonly.zrepo
"#
    );

    assert!(
        repo_config.git.is_empty(),
        "an omitted git section should default to empty"
    );
    assert!(repo_config.repos.is_empty());
    assert_eq!(repo_config.name, "snapshotonly");
    assert_eq!(zrepo::candidate_links(&repo_config).len(), 1);
}

#[test]
fn a_registry_may_omit_every_optional_section() {
    // Only `name` and `description` are genuinely required, so a minimal
    // manifest must remain valid.
    let repo_config = repo_config_with_zrepo_links(
        r#"
version: "2"
name: minimal
description: The smallest valid registry
zrepo:
  - type: main
    url: https://example.com/minimal.zrepo
"#
    );

    assert!(repo_config.pkg.is_empty());
    assert!(repo_config.delta.is_empty());
    assert!(repo_config.pgp.is_empty());
    assert!(repo_config.db.is_none());
    assert!(repo_config.advisory_prefix.is_none());
}

#[test]
fn repo_yaml_url_detection_only_matches_yaml_documents() {
    // A plain repository URL must not be mistaken for a manifest, otherwise
    // it would be requested verbatim and return HTML.
    assert!(!zoi_sync::is_repo_yaml_url("https://gitlab.com/org/repo"));
    assert!(!zoi_sync::is_repo_yaml_url(
        "https://github.com/org/repo.git"
    ));
    assert!(!zoi_sync::is_repo_yaml_url("/home/user/registry"));
    assert!(zoi_sync::is_repo_yaml_url("https://example.com/repo.yaml"));
    assert!(zoi_sync::is_repo_yaml_url("https://example.com/repo.yml"));
    assert!(zoi_sync::is_repo_yaml_url(
        "https://example.com/registry/repo.yaml?ref=main"
    ));
}

#[test]
fn a_registry_added_by_url_records_its_resolved_handle() -> Result<()> {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    // What `zoi registry add <url>` persists after resolving repo.yaml: the
    // real handle and metadata, with the manifest URL kept as the identity.
    config::add_resolved_registry(Registry {
        handle: "testreg".to_string(),
        url: "https://example.com/repo.yaml".to_string(),
        name: Some("Testreg".to_string()),
        description: Some("A test registry".to_string()),
        advisory_prefix: Some("TEST".to_string()),
        authorities: None
    })?;

    let cfg = config::read_config()?;
    let added = cfg
        .added_registries
        .iter()
        .find(|r| r.handle == "testreg")
        .expect("the resolved handle should have been recorded");

    assert_eq!(added.name.as_deref(), Some("Testreg"));
    assert_eq!(added.description.as_deref(), Some("A test registry"));
    assert_eq!(added.advisory_prefix.as_deref(), Some("TEST"));
    assert_eq!(added.url, "https://example.com/repo.yaml");

    Ok(())
}

#[test]
fn a_registry_with_no_name_cannot_be_added() {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    // A manifest without a `name` has no usable handle, so it must be
    // rejected rather than stored as an unidentifiable entry.
    let err = config::add_resolved_registry(Registry {
        handle: String::new(),
        url: "https://example.com/repo.yaml".to_string(),
        name: None,
        description: None,
        advisory_prefix: None,
        authorities: None
    })
    .expect_err("a registry without a name must be rejected");

    assert!(
        err.to_string().contains("declares no 'name'"),
        "unexpected error: {err}"
    );
}

#[test]
fn a_registry_cannot_be_added_twice() -> Result<()> {
    use zoi_core::config;
    use zoi_core::types::Registry;

    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("XDG_CONFIG_HOME", tmp.path().join("config"));

    let registry = Registry {
        handle: "testreg".to_string(),
        url: "https://example.com/repo.yaml".to_string(),
        name: Some("Testreg".to_string()),
        description: None,
        advisory_prefix: None,
        authorities: None
    };
    config::add_resolved_registry(registry.clone())?;

    // The same registry under a different URL spelling is still the same
    // handle, and must not be added twice.
    let mut other = registry.clone();
    other.url = "https://example.com/./repo.yaml".to_string();
    let err = config::add_resolved_registry(other)
        .expect_err("a duplicate registry must be rejected");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    // An identical URL is rejected too.
    let err = config::add_resolved_registry(registry)
        .expect_err("a duplicate registry must be rejected");
    assert!(
        err.to_string().contains("already exists"),
        "unexpected error: {err}"
    );

    Ok(())
}
