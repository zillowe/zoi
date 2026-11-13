//! Regression tests for uninstall state handling: exact shim ref-counting,
//! missing stored sources, autoremove scope and collection dependencies.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::pkg::{autoremove, db, local, resolve, types, uninstall, utils};

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

fn manifest(
    name: &str,
    version: &str,
    scope: types::Scope
) -> types::InstallManifest {
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
        scope,
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

/// Writes a manifest plus its `package.pkg.lua` snapshot into the store.
fn stage(m: &types::InstallManifest, root: &Path) {
    local::write_manifest(m).expect("unwrap failed");
    let source =
        root.join(format!("{}-{}-{:?}.pkg.lua", m.name, m.version, m.scope));
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
"#,
            m.name, m.repo, m.version
        )
    )
    .expect("unwrap failed");
    local::persist_package_source(m, &source).expect("unwrap failed");
}

fn is_installed(name: &str, scope: types::Scope) -> bool {
    let request = resolve::parse_source_string(name).expect("unwrap failed");
    !local::find_installed_manifests_matching(&request, scope)
        .expect("unwrap failed")
        .is_empty()
}

fn db_package(name: &str, bins: Option<Vec<&str>>) -> types::Package {
    types::Package {
        name: name.to_string(),
        repo: "core".to_string(),
        version: Some("1.0.0".to_string()),
        bins: bins.map(|b| b.into_iter().map(String::from).collect()),
        ..Default::default()
    }
}

#[test]
fn test_uninstall_removes_shim_when_only_substring_or_other_scope_matches() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let m = manifest("git", "1.0.0", types::Scope::User);
    stage(&m, &root);

    let conn = db::open_connection("local").expect("unwrap failed");
    let user = Some(types::Scope::User);
    db::update_package(
        &conn,
        &db_package("git", None),
        "local",
        user,
        None,
        None
    )
    .expect("unwrap failed");
    // `git-lfs` only contains the term; `sys-git` provides `git` in another
    // scope. Neither may keep the user-scope shim alive.
    db::update_package(
        &conn,
        &db_package("git-lfs", Some(vec!["git-lfs"])),
        "local",
        user,
        None,
        None
    )
    .expect("unwrap failed");
    db::update_package(
        &conn,
        &db_package("sys-git", Some(vec!["git"])),
        "local",
        Some(types::Scope::System),
        None,
        None
    )
    .expect("unwrap failed");
    drop(conn);

    let bin_dir = utils::get_user_bin_dir().expect("unwrap failed");
    fs::create_dir_all(&bin_dir).expect("unwrap failed");
    let shim = bin_dir.join("git");
    fs::write(&shim, "shim").expect("unwrap failed");

    uninstall::run("git", Some(types::Scope::User), true, true, false)
        .expect("unwrap failed");

    assert!(!shim.exists(), "shim should be removed");
    assert!(!is_installed("git", types::Scope::User));
}

#[test]
fn test_uninstall_keeps_shim_provided_by_other_package_in_same_scope() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let mut m = manifest("tool-a", "1.0.0", types::Scope::User);
    m.bins = Some(vec!["shared-tool".to_string()]);
    stage(&m, &root);

    let conn = db::open_connection("local").expect("unwrap failed");
    let user = Some(types::Scope::User);
    db::update_package(
        &conn,
        &db_package("tool-a", Some(vec!["shared-tool"])),
        "local",
        user,
        None,
        None
    )
    .expect("unwrap failed");
    db::update_package(
        &conn,
        &db_package("tool-b", Some(vec!["shared-tool"])),
        "local",
        user,
        None,
        None
    )
    .expect("unwrap failed");
    drop(conn);

    let bin_dir = utils::get_user_bin_dir().expect("unwrap failed");
    fs::create_dir_all(&bin_dir).expect("unwrap failed");
    let shim = bin_dir.join("shared-tool");
    fs::write(&shim, "shim").expect("unwrap failed");

    uninstall::run("tool-a", Some(types::Scope::User), true, true, false)
        .expect("unwrap failed");

    assert!(shim.exists(), "shim is still provided by tool-b");
}

#[test]
fn test_uninstall_without_stored_source_or_registry_still_succeeds() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let mut m = manifest("no-source", "1.0.0", types::Scope::User);
    m.installed_files = vec!["${pkgstore}/bin/no-source".to_string()];
    // Manifest only: no `package.pkg.lua` snapshot and no registry.
    local::write_manifest(&m).expect("unwrap failed");
    let vdir = local::get_package_version_dir(
        types::Scope::User,
        "local",
        "core",
        "no-source",
        "1.0.0"
    )
    .expect("unwrap failed");
    fs::create_dir_all(vdir.join("bin")).expect("unwrap failed");
    fs::write(vdir.join("bin/no-source"), "bin").expect("unwrap failed");

    uninstall::run("no-source", Some(types::Scope::User), true, true, false)
        .expect("uninstall should fall back to the install manifest");

    assert!(!is_installed("no-source", types::Scope::User));
    assert!(!vdir.exists());
}

#[test]
fn test_autoremove_removes_dependency_in_its_own_scope() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);
    let project = root.join("project");
    fs::create_dir_all(&project).expect("unwrap failed");
    ctx.set_current_dir(&project);

    // Orphaned dependency in user scope.
    let mut user_dep = manifest("auto-dep", "1.0.0", types::Scope::User);
    user_dep.reason = types::InstallReason::Dependency {
        parent: "#local@core/gone@1.0.0".to_string()
    };
    stage(&user_dep, &root);
    // Same package installed directly in the project scope, which uninstall
    // searches first when no scope is given.
    let project_pkg = manifest("auto-dep", "1.0.0", types::Scope::Project);
    stage(&project_pkg, &root);

    autoremove::run(true, false).expect("unwrap failed");

    assert!(
        !is_installed("auto-dep", types::Scope::User),
        "the orphaned user-scope dependency should be removed"
    );
    assert!(
        is_installed("auto-dep", types::Scope::Project),
        "the direct project-scope install must be left alone"
    );
}

#[test]
fn test_collection_uninstall_removes_its_zoi_dependencies_only() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("unwrap failed");
    let root = tmp.path().to_path_buf();
    isolate(&mut ctx, &root);

    let collection_id = "#local@core/my-coll@1.0.0";

    let mut dep = manifest("coll-dep", "1.0.0", types::Scope::User);
    dep.reason = types::InstallReason::Dependency {
        parent: collection_id.to_string()
    };
    stage(&dep, &root);
    let dep_dir =
        local::get_package_dir(types::Scope::User, "local", "core", "coll-dep")
            .expect("unwrap failed");
    local::add_dependent(&dep_dir, collection_id).expect("unwrap failed");

    let mut shared = manifest("coll-shared", "1.0.0", types::Scope::User);
    shared.reason = types::InstallReason::Dependency {
        parent: collection_id.to_string()
    };
    stage(&shared, &root);
    let shared_dir = local::get_package_dir(
        types::Scope::User,
        "local",
        "core",
        "coll-shared"
    )
    .expect("unwrap failed");
    local::add_dependent(&shared_dir, collection_id).expect("unwrap failed");
    // Another package still needs this one.
    local::add_dependent(&shared_dir, "#local@core/other-app@2.0.0")
        .expect("unwrap failed");

    // The collection has no stored source and no registry, so its package
    // type comes from the manifest fallback.
    let mut collection = manifest("my-coll", "1.0.0", types::Scope::User);
    collection.package_type = types::PackageType::Collection;
    collection.installed_dependencies = vec![
        "zoi:#local@core/coll-dep@1.0.0".to_string(),
        "zoi:#local@core/coll-shared@1.0.0".to_string(),
        "native:some-host-pkg".to_string(),
    ];
    local::write_manifest(&collection).expect("unwrap failed");

    uninstall::run("my-coll", Some(types::Scope::User), true, true, false)
        .expect("unwrap failed");

    assert!(!is_installed("my-coll", types::Scope::User));
    assert!(
        !is_installed("coll-dep", types::Scope::User),
        "the collection's own dependency should be removed"
    );
    assert!(
        is_installed("coll-shared", types::Scope::User),
        "a dependency still needed by another package must stay"
    );
    assert_eq!(
        local::get_dependents(&shared_dir).expect("unwrap failed"),
        vec!["#local@core/other-app@2.0.0".to_string()],
        "only the collection's link should be dropped"
    );
}
