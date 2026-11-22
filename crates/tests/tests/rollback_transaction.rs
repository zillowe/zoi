//! Rolling back a failed install transaction removes exactly the packages it
//! installed, in any order, without touching packages installed before it.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::pkg::{local, resolve, transaction, types, uninstall};

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
    common::TestContextGuard::set_pkg_dirs(Vec::new());
}

const SCOPE: types::Scope = types::Scope::User;

fn manifest(name: &str, deps: &[&str]) -> types::InstallManifest {
    types::InstallManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "local".to_string(),
        package_type: types::PackageType::Package,
        description: String::new(),
        reason: types::InstallReason::Direct,
        scope: SCOPE,
        bins: None,
        alternatives: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: deps
            .iter()
            .map(|d| format!("zoi:#local@core/{d}@1.0.0"))
            .collect(),
        dependencies_v2: None,
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        platform: zoi_core::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: vec![],
        file_digests: None,
        signature_verified: false,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

fn package_dir(name: &str) -> std::path::PathBuf {
    local::get_package_dir(SCOPE, "local", "core", name).expect("package dir")
}

/// Installs `m` into the store, with the dependent links an install records.
fn install(m: &types::InstallManifest, root: &Path) {
    local::write_manifest(m).expect("write manifest");
    if m.package_type != types::PackageType::Collection {
        let source = root.join(format!("{}.pkg.lua", m.name));
        fs::write(
            &source,
            format!(
                r#"metadata({{
  name = "{}",
  repo = "core",
  version = "1.0.0",
  description = "test",
  maintainer = {{ name = "Zoi", email = "zoi@example.com" }},
  types = {{ "source" }},
}})
"#,
                m.name
            )
        )
        .expect("write source");
        local::persist_package_source(m, &source).expect("persist source");
    }
    let id = local::installed_manifest_source(m);
    for dep in &m.installed_dependencies {
        let name = dep
            .trim_start_matches("zoi:#local@core/")
            .trim_end_matches("@1.0.0");
        local::add_dependent(&package_dir(name), &id).expect("link");
    }
}

fn is_installed(name: &str) -> bool {
    let request = resolve::parse_source_string(name).expect("request");
    !local::find_installed_manifests_matching(&request, SCOPE)
        .expect("lookup")
        .is_empty()
}

#[test]
fn rollback_removes_the_transactions_packages_and_keeps_older_ones() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    // Installed before the transaction.
    let pre = manifest("pre", &[]);
    install(&pre, &root);

    // Installed by the transaction, in dependency order: app needs lib,
    // and the collection needs all three (including the older `pre`).
    let lib = manifest("lib", &["pre"]);
    let app = manifest("app", &["lib"]);
    let mut coll = manifest("coll", &["app", "lib", "pre"]);
    coll.package_type = types::PackageType::Collection;

    let mut tx = transaction::begin().expect("begin");
    for m in [&lib, &app, &coll] {
        install(m, &root);
        transaction::record_operation(
            &mut tx,
            types::TransactionOperation::Install {
                manifest: Box::new(m.clone())
            }
        )
        .expect("record");
    }

    transaction::rollback(&tx.id).expect("rollback completes");

    for name in ["lib", "app", "coll"] {
        assert!(!is_installed(name), "{name} was not rolled back");
    }
    assert!(is_installed("pre"), "a package from before was removed");
    assert!(
        local::get_dependents(&package_dir("pre"))
            .expect("dependents")
            .is_empty(),
        "links from rolled-back packages remain on `pre`"
    );
}

#[test]
fn rolling_back_an_install_ignores_dependents_and_is_idempotent() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let lib = manifest("lib", &[]);
    let app = manifest("app", &["lib"]);
    install(&lib, &root);
    install(&app, &root);

    // Out of order on purpose: `app` still depends on `lib`. A normal
    // uninstall refuses this; a rollback removal must not.
    assert!(
        uninstall::run("lib", Some(SCOPE), true, true, false).is_err(),
        "a normal uninstall should still protect dependents"
    );
    uninstall::rollback_install(&lib, true).expect("rollback removal");
    assert!(!is_installed("lib"));

    // Already gone: running it again succeeds without doing anything.
    uninstall::rollback_install(&lib, true).expect("idempotent");
}
