//! Ephemeral environments (`zoi shell -p`, `zoi exec`) only skip installing a
//! package that is already installed exactly as resolved.

use std::collections::HashMap;
use std::fs;

use tempfile::tempdir;
use zoi::cmd::shell::{cleanup_ephemeral_installs, drop_already_installed};
use zoi::pkg::install::plan::InstallAction;
use zoi::pkg::install::resolver::InstallNode;
use zoi::pkg::{local, types};

mod common;

const HANDLE: &str = "zoidberg";
const REPO: &str = "core";
const NAME: &str = "tool";
const VERSION: &str = "1.0.0";

fn node(sub: Option<&str>, revision: &str) -> InstallNode {
    InstallNode {
        pkg: types::Package {
            name: NAME.to_string(),
            repo: REPO.to_string(),
            version: Some(VERSION.to_string()),
            scope: types::Scope::User,
            ..Default::default()
        },
        version: VERSION.to_string(),
        revision: revision.to_string(),
        sub_package: sub.map(str::to_string),
        repo_type: "official".to_string(),
        description: "test".to_string(),
        reason: types::InstallReason::Direct,
        source: NAME.to_string(),
        registry_handle: HANDLE.to_string(),
        chosen_options: vec![],
        chosen_optionals: vec![],
        dependencies: vec![],
        git_sha: None
    }
}

fn manifest(
    sub: Option<&str>,
    revision: &str,
    scope: types::Scope
) -> types::InstallManifest {
    types::InstallManifest {
        name: NAME.to_string(),
        version: VERSION.to_string(),
        epoch: 0,
        revision: revision.to_string(),
        sub_package: sub.map(str::to_string),
        repo: REPO.to_string(),
        repo_type: "official".to_string(),
        registry_handle: HANDLE.to_string(),
        package_type: types::PackageType::Package,
        description: "test".to_string(),
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
        platform: String::new(),
        service: None,
        installed_files: vec![],
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

/// Runs the filter for one resolved node and reports whether it is still
/// scheduled for installation.
fn still_planned(
    node: InstallNode,
    installed: &[types::InstallManifest]
) -> bool {
    let nodes = HashMap::from([("tool".to_string(), node)]);
    let mut plan =
        HashMap::from([("tool".to_string(), InstallAction::BuildAndInstall)]);
    drop_already_installed(&mut plan, &nodes, installed);
    plan.contains_key("tool")
}

/// Creates the user-scope version directory the node resolves to.
fn create_version_dir(ctx: &mut common::TestContextGuard) -> tempfile::TempDir {
    let tmp = tempdir().expect("tempdir");
    ctx.set_env_var("HOME", tmp.path());
    let dir = local::get_package_version_dir(
        types::Scope::User,
        HANDLE,
        REPO,
        NAME,
        VERSION
    )
    .expect("version dir");
    fs::create_dir_all(dir).expect("create version dir");
    tmp
}

#[test]
fn exact_installed_package_is_not_reinstalled() {
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let installed = [manifest(None, "1", types::Scope::User)];
    assert!(!still_planned(node(None, "1"), &installed));
}

#[test]
fn installed_older_revision_is_left_untouched() {
    // Reinstalling over a persistent installation from an ephemeral session
    // would replace its files without updating its records, so the
    // installed revision is used as-is.
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let installed = [manifest(None, "1", types::Scope::User)];
    assert!(!still_planned(node(None, "2"), &installed));
}

#[test]
fn copy_in_another_scope_does_not_count() {
    let mut ctx = common::TestContextGuard::acquire();
    // The user-scope version directory exists (e.g. for another
    // sub-package), but this sub-package is only installed in project scope.
    let _home = create_version_dir(&mut ctx);
    let installed = [
        manifest(Some("main"), "1", types::Scope::User),
        manifest(Some("extra"), "1", types::Scope::Project)
    ];
    assert!(still_planned(node(Some("extra"), "1"), &installed));
}

#[test]
fn other_sub_package_does_not_count() {
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let installed = [manifest(Some("main"), "1", types::Scope::User)];
    assert!(still_planned(node(Some("extra"), "1"), &installed));
    assert!(!still_planned(node(Some("main"), "1"), &installed));
}

/// Returns the user-scope version directory used by these tests.
fn version_dir() -> std::path::PathBuf {
    local::get_package_version_dir(
        types::Scope::User,
        HANDLE,
        REPO,
        NAME,
        VERSION
    )
    .expect("version dir")
}

/// A manifest whose store file is `bin/<file>`.
fn manifest_with_file(
    sub: Option<&str>,
    scope: types::Scope,
    file: &str
) -> types::InstallManifest {
    let mut m = manifest(sub, "1", scope);
    m.installed_files = vec![format!("${{pkgstore}}/bin/{file}")];
    m
}

#[test]
fn cleanup_keeps_a_persistent_sub_package_sharing_the_directory() {
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let bin = version_dir().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("main-tool"), "main").expect("main file");
    fs::write(bin.join("extra-tool"), "extra").expect("extra file");

    // `main` is installed in user scope and `extra` only in project scope;
    // the session installed a user-scope `extra` into the shared directory.
    let before = [
        manifest_with_file(Some("main"), types::Scope::User, "main-tool"),
        manifest_with_file(Some("extra"), types::Scope::Project, "extra-tool")
    ];
    let session = vec![manifest_with_file(
        Some("extra"),
        types::Scope::User,
        "extra-tool"
    )];

    cleanup_ephemeral_installs(session, &before, false);

    assert!(
        !bin.join("extra-tool").exists(),
        "session file must be removed"
    );
    assert!(bin.join("main-tool").exists(), "persistent file must stay");
}

#[test]
fn cleanup_removes_an_unshared_session_install() {
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let dir = version_dir();
    fs::write(dir.join("marker"), "x").expect("marker");

    let session = vec![manifest_with_file(None, types::Scope::User, "tool")];
    cleanup_ephemeral_installs(session, &[], false);

    assert!(!dir.exists(), "the session's version directory is removed");
}

#[test]
fn cleanup_leaves_a_persistent_install_untouched() {
    let mut ctx = common::TestContextGuard::acquire();
    let _home = create_version_dir(&mut ctx);
    let dir = version_dir();
    fs::write(dir.join("marker"), "x").expect("marker");

    let installed = manifest_with_file(None, types::Scope::User, "tool");
    cleanup_ephemeral_installs(
        vec![installed.clone()],
        std::slice::from_ref(&installed),
        false
    );

    assert!(dir.join("marker").exists());
}

#[cfg(unix)]
#[test]
fn cleanup_unlinks_session_symlinks_in_a_shared_directory() {
    use std::os::unix::fs::symlink;

    let mut ctx = common::TestContextGuard::acquire();
    let home = create_version_dir(&mut ctx);
    let bin = version_dir().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("main-tool"), "main").expect("main file");

    let outside = home.path().join("outside-target");
    fs::write(&outside, "outside").expect("outside target");
    symlink(&outside, bin.join("outside-link")).expect("outside link");
    symlink(home.path().join("missing"), bin.join("dangling-link"))
        .expect("dangling link");

    let before = [manifest_with_file(
        Some("main"),
        types::Scope::User,
        "main-tool"
    )];
    let mut session_manifest =
        manifest_with_file(Some("extra"), types::Scope::User, "outside-link");
    session_manifest
        .installed_files
        .push("${pkgstore}/bin/dangling-link".to_string());

    cleanup_ephemeral_installs(vec![session_manifest], &before, false);

    assert!(fs::symlink_metadata(bin.join("outside-link")).is_err());
    assert!(fs::symlink_metadata(bin.join("dangling-link")).is_err());
    assert!(outside.exists(), "the link target must not be touched");
    assert!(bin.join("main-tool").exists(), "persistent file must stay");
}
