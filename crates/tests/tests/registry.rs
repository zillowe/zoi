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
