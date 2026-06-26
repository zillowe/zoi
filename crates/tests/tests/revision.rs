use tempfile::tempdir;
use zoi::pkg::{config, db, local, types};

mod common;

#[test]
fn test_package_outdated_on_revision_bump() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());
    ctx.set_env_var("ZOI_DB_DIR", root.join("db"));

    let pkg_name = "rev-test";
    let version = "1.0.0";
    let handle = "testreg";
    let repo = "core";

    let cfg = types::Config {
        added_registries: vec![types::Registry {
            handle: handle.to_string(),
            url: "https://example.com/testreg.git".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let db_root = root.join("db");
    let reg_root = db_root.join(handle);
    std::fs::create_dir_all(reg_root.join(repo).join(pkg_name)).unwrap();
    std::fs::write(
        reg_root.join("repo.yaml"),
        "name: testreg\nrepos: [{name: core, type: official, active: true}]",
    )
    .unwrap();
    std::fs::write(
        reg_root.join(repo).join(pkg_name).join(format!("{}.pkg.lua", pkg_name)),
        format!("metadata({{ name = '{}', repo = '{}', version = '{}', revision = '2', description = 'test', maintainer = {{ name = 'test', email = 'test' }}, types = {{ 'source' }} }})", pkg_name, repo, version)
    ).unwrap();

    let manifest = types::InstallManifest {
        name: pkg_name.to_string(),
        version: version.to_string(),
        revision: "1".to_string(),
        sub_package: None,
        repo: repo.to_string(),
        registry_handle: handle.to_string(),
        package_type: types::PackageType::Package,
        reason: types::InstallReason::Direct,
        scope: types::Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("test".to_string()),
        service: None,
        installed_files: vec![],
        installed_size: None,
        sandbox: None,
    };
    local::write_manifest(&manifest).unwrap();

    let conn = db::open_connection(handle).unwrap();
    let pkg_meta = types::Package {
        name: pkg_name.to_string(),
        repo: repo.to_string(),
        version: Some(version.to_string()),
        revision: "2".to_string(),
        ..Default::default()
    };
    db::update_package(&conn, &pkg_meta, handle, None, None, None).unwrap();

    let source = format!("#{}@{}/{}", handle, repo, pkg_name);
    let (resolved_pkg, new_version, _, _, _, _) =
        zoi::pkg::resolve::resolve_package_and_version(&source, true, true).unwrap();

    assert_eq!(new_version, version);
    assert_eq!(resolved_pkg.revision, "2");
    assert_ne!(manifest.revision, resolved_pkg.revision);

    let is_outdated = manifest.version != new_version || manifest.revision != resolved_pkg.revision;
    assert!(
        is_outdated,
        "Package should be considered outdated due to revision bump"
    );
}
