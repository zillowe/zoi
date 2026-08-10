//! Integration tests for package uninstallation logic.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::cli::InstallScope;
use zoi::cmd;
use zoi::pkg::{local, plugin, resolve, types, uninstall};

mod common;

fn sample_manifest(name: &str, repo: &str) -> types::InstallManifest {
    types::InstallManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: repo.to_string(),
        repo_type: "official".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: String::new(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        dependencies_v2: None,
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        platform: zoi_core::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: vec![],
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

fn sample_manifest_in_scope(
    name: &str,
    repo: &str,
    scope: types::Scope
) -> types::InstallManifest {
    let mut manifest = sample_manifest(name, repo);
    manifest.scope = scope;
    manifest
}

fn write_package_source(
    path: &Path,
    name: &str,
    repo: &str,
    version: &str,
    uninstall_target: Option<&str>
) {
    let uninstall_ops = uninstall_target.map_or_else(
        || "local __ZoiUninstallOperations = {}".to_string(),
        |target| {
            format!(
                r#"__ZoiUninstallOperations = {{
  {{ op = "zrm", path = "{target}" }},
}}"#
            )
        }
    );

    fs::write(
        path,
        format!(
            r#"metadata({{
  name = "{name}",
  repo = "{repo}",
  version = "{version}",
  description = "test",
  maintainer = {{ name = "Zoi", email = "zoi@example.com" }},
  types = {{ "source" }},
}})

{uninstall_ops}

function uninstall(_args)
end
"#
        )
    )
    .expect("unwrap failed");
}

#[test]
fn test_uninstall_uses_stored_package_source_over_registry_drift() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    let db_root = root.join("db");
    ctx.set_env_var("HOME", &root);
    ctx.set_env_var("ZOI_DB_DIR", &db_root);
    common::TestContextGuard::set_sysroot(root.clone());

    let manifest = sample_manifest("test-pkg", "core");
    local::write_manifest(&manifest).expect("unwrap failed");

    let stored_hit = root.join("stored-hit");
    let db_hit = root.join("db-hit");
    fs::write(&stored_hit, "stored").expect("unwrap failed");
    fs::write(&db_hit, "db").expect("unwrap failed");

    let stored_source = root.join("stored.pkg.lua");
    write_package_source(
        &stored_source,
        "test-pkg",
        "core",
        "1.0.0",
        Some("${usrhome}/stored-hit")
    );
    local::persist_package_source(&manifest, &stored_source)
        .expect("unwrap failed");

    let db_pkg_dir = db_root.join("local").join("core").join("test-pkg");
    fs::create_dir_all(&db_pkg_dir).expect("unwrap failed");
    write_package_source(
        &db_pkg_dir.join("test-pkg.pkg.lua"),
        "test-pkg",
        "core",
        "1.0.0",
        Some("${usrhome}/db-hit")
    );

    uninstall::run("test-pkg", Some(types::Scope::User), true, false, false)
        .expect("unwrap failed");

    assert!(
        !stored_hit.exists(),
        "stored uninstall script should have been used"
    );
    assert!(
        db_hit.exists(),
        "registry drift script should not be consulted when a stored source \
         exists"
    );
}

#[test]
fn test_uninstall_requires_explicit_source_for_ambiguous_name_matches() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    local::write_manifest(&sample_manifest("shared", "core"))
        .expect("unwrap failed");
    local::write_manifest(&sample_manifest("shared", "extra"))
        .expect("unwrap failed");

    let err =
        uninstall::run("shared", Some(types::Scope::User), true, false, false)
            .expect_err("unwrap_err failed");
    assert!(err.to_string().contains("ambiguous"));
}

#[test]
fn test_uninstall_explicit_source_removes_only_matching_install() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    let core_manifest = sample_manifest("shared", "core");
    let extra_manifest = sample_manifest("shared", "extra");
    local::write_manifest(&core_manifest).expect("unwrap failed");
    local::write_manifest(&extra_manifest).expect("unwrap failed");

    let core_source = root.join("core.pkg.lua");
    write_package_source(&core_source, "shared", "core", "1.0.0", None);
    local::persist_package_source(&core_manifest, &core_source)
        .expect("unwrap failed");

    let extra_source = root.join("extra.pkg.lua");
    write_package_source(&extra_source, "shared", "extra", "1.0.0", None);
    local::persist_package_source(&extra_manifest, &extra_source)
        .expect("unwrap failed");

    uninstall::run(
        "#local@extra/shared@1.0.0",
        Some(types::Scope::User),
        true,
        false,
        false
    )
    .expect("unwrap failed");

    let core_request = resolve::parse_source_string("#local@core/shared@1.0.0")
        .expect("unwrap failed");
    let extra_request =
        resolve::parse_source_string("#local@extra/shared@1.0.0")
            .expect("unwrap failed");

    assert_eq!(
        local::find_installed_manifests_matching(
            &core_request,
            types::Scope::User
        )
        .expect("unwrap failed")
        .len(),
        1
    );
    assert!(
        local::find_installed_manifests_matching(
            &extra_request,
            types::Scope::User
        )
        .expect("unwrap failed")
        .is_empty()
    );
}

#[test]
fn test_cmd_uninstall_respects_scope_override_when_names_overlap() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    let user_manifest =
        sample_manifest_in_scope("shared", "core", types::Scope::User);
    let system_manifest =
        sample_manifest_in_scope("shared", "core", types::Scope::System);
    local::write_manifest(&user_manifest).expect("unwrap failed");
    local::write_manifest(&system_manifest).expect("unwrap failed");

    let user_source = root.join("user.pkg.lua");
    write_package_source(&user_source, "shared", "core", "1.0.0", None);
    local::persist_package_source(&user_manifest, &user_source)
        .expect("unwrap failed");

    let system_source = root.join("system.pkg.lua");
    write_package_source(&system_source, "shared", "core", "1.0.0", None);
    local::persist_package_source(&system_manifest, &system_source)
        .expect("unwrap failed");

    let plugin_manager = plugin::PluginManager::new().expect("unwrap failed");
    cmd::uninstall::run(
        &[String::from("shared")],
        Some(InstallScope::User),
        false,
        false,
        false,
        true,
        false,
        Some(&plugin_manager),
        false,
        false,
        false
    )
    .expect("unwrap failed");

    assert!(
        local::is_package_installed("shared", None, types::Scope::User)
            .expect("unwrap failed")
            .is_none()
    );
    assert!(
        local::is_package_installed("shared", None, types::Scope::System)
            .expect("unwrap failed")
            .is_some()
    );
}
