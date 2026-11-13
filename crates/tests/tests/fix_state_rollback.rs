//! Regression tests for `zoi rollback` and transaction rollback restoring
//! package state from the store.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::pkg::{db, local, resolve, rollback, transaction, types, utils};

mod common;

fn isolate(ctx: &mut common::TestContextGuard, root: &Path) {
    ctx.set_env_var("HOME", root);
    ctx.set_env_var("XDG_DATA_HOME", root.join(".local/share"));
    ctx.set_env_var("XDG_STATE_HOME", root.join(".local/state"));
    ctx.set_env_var("XDG_CONFIG_HOME", root.join(".config"));
    ctx.set_env_var("XDG_CACHE_HOME", root.join(".cache"));
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));
    ctx.set_env_var("ZOI_TEST_SKIP_SERVICE_COMMANDS", "1");
    common::TestContextGuard::set_sysroot(root.to_path_buf());
    common::TestContextGuard::set_offline(true);
}

fn manifest(name: &str, version: &str) -> types::InstallManifest {
    types::InstallManifest {
        name: name.to_string(),
        version: version.to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: String::new(),
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        alternatives: None,
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
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

/// Writes a stored version (manifest + `package.pkg.lua` snapshot) and makes
/// it the active one.
fn stage(m: &types::InstallManifest, root: &Path) {
    local::write_manifest(m).expect("unwrap failed");
    let source = root.join(format!("{}-{}.pkg.lua", m.name, m.version));
    fs::write(
        &source,
        format!(
            r#"metadata({{
  name = "{}",
  repo = "{}",
  version = "{}",
  description = "test",
  maintainer = {{ name = "Zoi", email = "zoi@example.com" }},
  types = {{ "source" }},
}})

function uninstall(_args)
end
"#,
            m.name, m.repo, m.version
        )
    )
    .expect("unwrap failed");
    local::persist_package_source(m, &source).expect("unwrap failed");
}

fn version_dir(name: &str, version: &str) -> std::path::PathBuf {
    local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        name,
        version
    )
    .expect("unwrap failed")
}

fn active_version(name: &str) -> Option<String> {
    let request = resolve::parse_source_string(name).expect("unwrap failed");
    local::find_installed_manifests_matching(&request, types::Scope::User)
        .expect("unwrap failed")
        .first()
        .map(|m| m.version.clone())
}

#[test]
fn test_rollback_picks_highest_lower_version_and_ignores_junk() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let mut v100 = manifest("rb-pkg", "1.0.0");
    v100.bins = Some(vec!["rb-tool".to_string()]);
    let mut v120 = manifest("rb-pkg", "1.2.0");
    v120.bins = Some(vec!["rb-tool".to_string()]);
    let mut v110 = manifest("rb-pkg", "1.1.0");
    v110.bins = Some(vec!["rb-tool".to_string()]);
    v110.installed_files = vec!["${pkgstore}/bin/rb-tool".to_string()];
    stage(&v100, &root);
    stage(&v120, &root);
    // 1.1.0 is active while a newer 1.2.0 is also stored.
    stage(&v110, &root);
    let tool = version_dir("rb-pkg", "1.1.0").join("bin/rb-tool");
    fs::create_dir_all(tool.parent().expect("parent")).expect("unwrap failed");
    fs::write(&tool, "tool").expect("unwrap failed");
    assert_eq!(active_version("rb-pkg").as_deref(), Some("1.1.0"));

    // Junk next to the version directories must never be picked.
    let package_dir =
        local::get_package_dir(types::Scope::User, "local", "core", "rb-pkg")
            .expect("unwrap failed");
    let staging = package_dir.join(".tmp-install-abc123");
    fs::create_dir_all(&staging).expect("unwrap failed");
    let mut staged = manifest("rb-pkg", "1.0.9");
    staged.version = ".tmp-install-abc123".to_string();
    fs::write(
        staging.join("manifest.json"),
        serde_json::to_string(&staged).expect("unwrap failed")
    )
    .expect("unwrap failed");
    fs::write(package_dir.join("config.toml.zoisave"), "saved")
        .expect("unwrap failed");
    fs::create_dir_all(package_dir.join("1.0.5")).expect("unwrap failed");

    rollback::run("rb-pkg", true).expect("rollback should succeed");

    // Highest version lower than 1.1.0, not the newer 1.2.0.
    assert_eq!(active_version("rb-pkg").as_deref(), Some("1.0.0"));
    assert!(!version_dir("rb-pkg", "1.1.0").exists());
    assert!(version_dir("rb-pkg", "1.2.0").exists());
    assert!(package_dir.join("config.toml.zoisave").exists());

    // The shim is restored for the activated version.
    let shim = utils::get_user_bin_dir()
        .expect("unwrap failed")
        .join("rb-tool");
    assert!(shim.is_symlink(), "shim should be restored by rollback");

    // The database row follows the activated version.
    let conn = db::open_connection("local").expect("unwrap failed");
    let version: String = conn
        .query_row(
            "SELECT version FROM packages WHERE name = 'rb-pkg' AND scope = \
             'user'",
            [],
            |row| row.get(0)
        )
        .expect("database row should exist after rollback");
    assert_eq!(version, "1.0.0");
}

#[test]
fn test_rollback_without_older_version_errors_and_changes_nothing() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    stage(&manifest("newer-only", "2.0.0"), &root);
    stage(&manifest("newer-only", "1.0.0"), &root);
    assert_eq!(active_version("newer-only").as_deref(), Some("1.0.0"));

    let err = rollback::run("newer-only", true)
        .expect_err("only a newer version is stored");
    assert!(
        err.to_string().contains("No previous version"),
        "unexpected error: {err}"
    );
    assert_eq!(active_version("newer-only").as_deref(), Some("1.0.0"));
    assert!(version_dir("newer-only", "2.0.0").exists());
}

#[test]
fn test_rollback_refuses_when_previous_version_has_files_outside_store() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let shared = "${usrroot}/usr/share/rb-sys/data.txt".to_string();
    let mut v1 = manifest("rb-sys", "1.0.0");
    v1.installed_files = vec![shared.clone()];
    let mut v2 = manifest("rb-sys", "2.0.0");
    v2.installed_files = vec![shared.clone()];
    stage(&v1, &root);
    stage(&v2, &root);

    let on_disk = std::path::PathBuf::from(
        utils::expand_placeholders(
            &shared,
            &version_dir("rb-sys", "2.0.0"),
            types::Scope::User
        )
        .expect("unwrap failed")
    );
    fs::create_dir_all(on_disk.parent().expect("parent"))
        .expect("unwrap failed");
    fs::write(&on_disk, "v2 data").expect("unwrap failed");

    let err = rollback::run("rb-sys", true)
        .expect_err("files outside the store cannot be restored");
    assert!(
        err.to_string().contains("outside the Zoi store"),
        "unexpected error: {err}"
    );

    // Nothing was touched.
    assert_eq!(active_version("rb-sys").as_deref(), Some("2.0.0"));
    assert_eq!(
        fs::read_to_string(&on_disk).expect("unwrap failed"),
        "v2 data"
    );
    assert!(version_dir("rb-sys", "2.0.0").exists());
}

#[test]
fn test_transaction_rollback_restores_pkgstore_only_package_from_store() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let mut m = manifest("tx-store-pkg", "1.0.0");
    m.installed_files = vec!["${pkgstore}/bin/tx-store-pkg".to_string()];
    stage(&m, &root);

    // Simulate the uninstall half of a transaction: the active manifest is
    // gone but the version directory is still in the store.
    let vdir = version_dir("tx-store-pkg", "1.0.0");
    let mut trans = transaction::begin().expect("unwrap failed");
    let id = trans.id.clone();
    transaction::record_operation(
        &mut trans,
        types::TransactionOperation::Uninstall {
            manifest: Box::new(m.clone())
        }
    )
    .expect("unwrap failed");

    // Offline: a registry reinstall would fail, so success proves the store
    // path was taken for `${pkgstore}` entries.
    transaction::rollback(&id).expect("rollback should restore from store");
    assert_eq!(active_version("tx-store-pkg").as_deref(), Some("1.0.0"));
    assert!(vdir.exists());
}

#[test]
fn test_transaction_logs_are_written_atomically_and_corrupt_logs_skipped() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let m = manifest("tx-log-pkg", "1.0.0");
    let mut trans = transaction::begin().expect("unwrap failed");
    let id = trans.id.clone();
    transaction::record_operation(
        &mut trans,
        types::TransactionOperation::Install {
            manifest: Box::new(m)
        }
    )
    .expect("unwrap failed");

    let dir = utils::get_user_state_dir()
        .expect("unwrap failed")
        .join("transactions");
    let names: Vec<String> = fs::read_dir(&dir)
        .expect("unwrap failed")
        .map(|e| {
            e.expect("unwrap failed")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .filter(|n| n != "history")
        .collect();
    assert_eq!(names, vec![format!("{id}.json")]);

    // A truncated log must not hide the others.
    fs::write(
        dir.join("0190a000-0000-7000-8000-000000000000.json"),
        "{ \"id\": "
    )
    .expect("unwrap failed");
    let listed = transaction::list_transactions()
        .expect("corrupt logs are skipped, not fatal");
    assert!(listed.iter().any(|t| t.id == id));
}
