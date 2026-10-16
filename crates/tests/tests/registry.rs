//! Integration tests for Zoi package registry operations.

use std::fs;

use anyhow::Result;
use chrono::Datelike;
use tempfile::TempDir;
use zoi::pkg::registry;

#[test]
fn test_registry_init() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().join("my-reg");

    registry::init(&path)?;

    assert!(path.join("repo.yaml").exists());
    assert!(path.join("packages.json").exists());
    assert!(path.join("advisories.json").exists());
    assert!(path.join("core").is_dir());
    assert!(path.join("main").is_dir());

    let repo_yaml = fs::read_to_string(path.join("repo.yaml"))?;
    assert!(repo_yaml.contains("My-Registry"));
    assert!(repo_yaml.contains(
        "zillowe.qzz.io/docs/zds/zoi/repositories#the-repoyaml-file"
    ));

    Ok(())
}

#[test]
fn test_registry_add_package() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let reg_path = temp_dir.path();

    registry::init(reg_path)?;
    registry::add_package(reg_path, Some("test-pkg"), Some("community"))?;

    let pkg_lua_path = reg_path.join("community/test-pkg/test-pkg.pkg.lua");
    assert!(pkg_lua_path.exists());

    let content = fs::read_to_string(pkg_lua_path)?;
    assert!(content.contains("name = \"test-pkg\""));
    assert!(content.contains("repo = \"community\""));
    assert!(content.contains("zillowe.qzz.io/docs/zds/zoi/creating-packages"));

    Ok(())
}

#[test]
fn test_registry_check() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let reg_path = temp_dir.path();

    registry::init(reg_path)?;

    registry::add_package(reg_path, Some("valid"), Some("core"))?;

    let broken_pkg_dir = reg_path.join("core/broken");
    fs::create_dir_all(&broken_pkg_dir)?;
    let broken_lua = r#"
metadata({
  name = "broken",
  repo = "core",
  description = "missing version",
  maintainer = { name = "test", email = "test" },
  types = { "source" }
})
"#;
    fs::write(broken_pkg_dir.join("broken.pkg.lua"), broken_lua)?;

    let result = registry::check(reg_path);
    assert!(result.is_err());
    assert!(
        result
            .expect_err("unwrap_err failed")
            .to_string()
            .contains("error(s)")
    );

    Ok(())
}

#[test]
fn test_registry_advisory_id_assignment() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let reg_path = temp_dir.path();

    registry::init(reg_path)?;
    registry::add_package(reg_path, Some("vuln-pkg"), Some("core"))?;

    let current_year = chrono::Utc::now().year();
    let temp_adv_path = reg_path
        .join("core/vuln-pkg")
        .join(format!("ZSA-{current_year}-TEMP.sec.yaml"));

    let adv_content = r#"
package: "vuln-pkg"
summary: "Test vulnerability"
severity: "high"
affected_range: "<1.0.0"
fixed_in: "1.0.0"
description: "Test"
"#
    .to_string();
    fs::write(&temp_adv_path, adv_content)?;

    let repo_yaml_content = r#"
name: "Test-Reg"
description: "Test"
handle: "testreg"
advisory_prefix: "TEST"
git:
  - type: main
    url: "https://example.com"
repos:
  - name: core
    type: official
    active: true
"#;
    fs::write(reg_path.join("repo.yaml"), repo_yaml_content)?;

    registry::generate_metadata(reg_path)?;

    let expected_id = format!("TEST-{current_year}-C0001");
    let final_adv_path = reg_path
        .join("core/vuln-pkg")
        .join(format!("{expected_id}.sec.yaml"));

    assert!(
        final_adv_path.exists(),
        "Advisory file should be renamed to its ID"
    );

    let final_content = fs::read_to_string(final_adv_path)?;
    assert!(final_content.contains(&format!("id: {expected_id}")));

    let advisories_json = fs::read_to_string(reg_path.join("advisories.json"))?;
    assert!(
        advisories_json.contains(&expected_id),
        "advisories.json should key the advisory by its full ID"
    );
    assert!(
        advisories_json.contains("vuln-pkg"),
        "advisories.json should point to the advisory file path"
    );
    assert!(
        advisories_json.contains("\"version\": \"2\""),
        "advisories.json should use the current format version"
    );

    Ok(())
}

/// Builds a minimal but valid registry tree and returns its root.
fn make_registry_tree(root: &std::path::Path, version: &str) -> Result<()> {
    fs::create_dir_all(root.join("main/ripgrep"))?;
    fs::write(
        root.join("repo.yaml"),
        "version: \"2\"\nname: snapshotreg\ndescription: Snapshot test \
         registry\ngit: []\nrepos: []\n"
    )?;
    fs::write(
        root.join("packages.json"),
        "{\"version\":\"2\",\"packages\":{}}"
    )?;
    fs::write(
        root.join("main/ripgrep/ripgrep.pkg.lua"),
        format!(
            "metadata({{name=\"ripgrep\",repo=\"main\",version=\"{version}\"\
             }})\n"
        )
    )?;
    Ok(())
}

#[test]
fn test_registry_zrepo_builds_a_publishable_snapshot() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let tree = temp_dir.path().join("registry");
    make_registry_tree(&tree, "14.1.0")?;

    let output = temp_dir.path().join("snapshotreg.zrepo");
    let stats = zoi_sync::zrepo::create_zrepo(&tree, &output, None)?;

    assert!(output.exists(), "the .zrepo should be written");
    assert_eq!(stats.file_count, 3, "all three registry files are included");

    // The sidecars are what a client verifies the download against, and they
    // are named after the full artifact so the `hash` URL in repo.yaml reads
    // the same way it does for .zpa and .zdelta.
    let hash =
        fs::read_to_string(temp_dir.path().join("snapshotreg.zrepo.hash"))?;
    assert_eq!(hash.trim(), stats.sha256);
    assert!(temp_dir.path().join("snapshotreg.zrepo.size").exists());

    // The payload must be a real zstd-compressed tar of the tree, since that is
    // exactly what a client expects to download.
    let raw = fs::read(&output)?;
    let tar_bytes = zstd::stream::decode_all(&raw[..])?;
    assert!(
        tar_bytes.windows(5).any(|w| w == b"ustar"),
        "the snapshot payload should be a tar archive"
    );
    let payload = String::from_utf8_lossy(&tar_bytes);
    assert!(
        payload.contains("snapshotreg"),
        "repo.yaml should be inside"
    );
    assert!(
        payload.contains("ripgrep.pkg.lua"),
        "packages should be inside"
    );

    Ok(())
}

#[test]
fn test_registry_zrepo_publishes_a_zsync_control_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let tree = temp_dir.path().join("registry");
    make_registry_tree(&tree, "14.1.0")?;

    // Bulk packages so the snapshot has more than one frame to recover.
    for index in 0..200 {
        let dir = tree.join(format!("main/tool-{index}"));
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join(format!("tool-{index}.pkg.lua")),
            format!(
                "metadata({{name=\"tool-{index}\",repo=\"main\",version=\"1.0.\
                 {index}\"}})\n"
            )
        )?;
    }

    let published = temp_dir.path().join("main.zrepo");
    zoi_sync::zrepo::create_zrepo(&tree, &published, None)?;

    let control_path = temp_dir.path().join("main.zrepo.zsync");
    assert!(
        control_path.exists(),
        "a snapshot must be published with the control file that makes it \
         transferable"
    );

    let control = zync::ControlFile::parse(fs::File::open(&control_path)?)?;
    assert_eq!(
        control.length,
        fs::metadata(&published)?.len(),
        "the control file must describe the snapshot beside it"
    );

    // The URL is relative, so one control file serves a primary and every
    // mirror without the publisher knowing where it will be hosted.
    assert_eq!(control.urls, vec!["main.zrepo".to_string()]);

    Ok(())
}

#[test]
fn test_registry_zrepo_excludes_git_and_prior_snapshots() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let tree = temp_dir.path().join("registry");
    make_registry_tree(&tree, "14.1.0")?;

    // Output inside the tree must not end up inside the snapshot it produces,
    // or every rebuild would nest the previous snapshot inside the next one.
    fs::create_dir_all(tree.join(".git/objects"))?;
    fs::write(tree.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    fs::write(tree.join("stale.zrepo"), "stale\n")?;

    let output = temp_dir.path().join("out.zrepo");
    let stats = zoi_sync::zrepo::create_zrepo(&tree, &output, None)?;
    assert_eq!(stats.file_count, 3, "only registry content is published");

    let tar_bytes = zstd::stream::decode_all(&fs::read(&output)?[..])?;
    let payload = String::from_utf8_lossy(&tar_bytes);
    assert!(!payload.contains(".git"), "git plumbing must be excluded");
    assert!(
        !payload.contains("stale"),
        "a previous snapshot must be excluded"
    );

    Ok(())
}

#[test]
fn test_registry_zrepo_requires_a_registry_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let not_a_registry = temp_dir.path().join("random");
    fs::create_dir_all(&not_a_registry)?;
    fs::write(not_a_registry.join("readme.md"), "hello\n")?;

    let err = zoi_sync::zrepo::create_zrepo(
        &not_a_registry,
        &temp_dir.path().join("out.zrepo"),
        None
    )
    .expect_err("a directory without repo.yaml must be rejected");

    assert!(
        err.to_string().contains("no repo.yaml"),
        "unexpected error: {err}"
    );
    Ok(())
}

/// A hostile `advisory_prefix` in `repo.yaml` must not be able to redirect
/// `zoi reg build` writes outside the registry root.
///
/// The prefix is interpolated into the advisory file name
/// (`{prefix}-{year}-{sev}{n}.sec.yaml`), so a prefix carrying a path
/// separator or `..` would otherwise let a crafted registry write files to
/// arbitrary locations relative to the registry directory.
#[test]
fn test_registry_build_rejects_traversal_advisory_prefix() -> Result<()> {
    for hostile_prefix in ["../escape", "sub/dir", "..", "", "ZSA/../../x"] {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("my-reg");
        registry::init(&path)?;

        // Rewrite repo.yaml with a prefix that tries to break out.
        let repo_yaml = fs::read_to_string(path.join("repo.yaml"))?;
        let repo_yaml = repo_yaml.replace(
            "advisory_prefix: \"RSA\"",
            &format!("advisory_prefix: \"{hostile_prefix}\"")
        );
        fs::write(path.join("repo.yaml"), repo_yaml)?;

        let err = registry::generate_metadata(&path).expect_err(
            "a traversal advisory_prefix must be rejected before any write"
        );

        assert!(
            err.to_string().contains("advisory_prefix"),
            "prefix '{hostile_prefix}' produced an unexpected error: {err}"
        );
    }

    Ok(())
}

/// The benign default prefix must keep working, so the hardening above does
/// not regress normal advisory generation.
#[test]
fn test_registry_build_accepts_default_advisory_prefix() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path = temp_dir.path().join("my-reg");
    registry::init(&path)?;

    registry::generate_metadata(&path)?;

    assert!(path.join("advisories.json").exists());
    Ok(())
}
