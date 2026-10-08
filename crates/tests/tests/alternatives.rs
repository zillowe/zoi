//! Integration coverage for the alternatives mechanism driven through a
//! sysroot.
//!
//! The unit tests in `zoi-core` exercise the registry directly. These go
//! through the real path a package install takes: a manifest written into the
//! package store, `reconcile` reading it back, and symlinks appearing in the
//! target tree. That path is the one that could plausibly break silently,
//! because a manifest that is never found simply registers nothing and reports
//! no error.

use std::fs;
use std::path::Path;

use zoi::pkg::types::{
    AlternativeEntry, InstallManifest, InstallReason, PackageType, Scope
};

/// Builds a manifest declaring the given alternatives.
///
/// Every field is spelled out because `InstallManifest` has no `Default`, which
/// is deliberate: a silently-default manifest would let a test pass while
/// omitting something the code under test actually reads.
fn manifest_with(
    name: &str,
    alternatives: Vec<AlternativeEntry>
) -> InstallManifest {
    InstallManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        epoch: 0,
        revision: "1".to_string(),
        sub_package: None,
        repo: "core".to_string(),
        repo_type: "official".to_string(),
        registry_handle: "zoidberg".to_string(),
        package_type: PackageType::Package,
        description: "test package".to_string(),
        reason: InstallReason::Direct,
        scope: Scope::System,
        bins: None,
        alternatives: if alternatives.is_empty() {
            None
        } else {
            Some(alternatives)
        },
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        dependencies_v2: None,
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("pre-compiled".to_string()),
        platform: zoi::utils::get_platform().unwrap_or_default(),
        service: None,
        installed_files: vec![],
        file_digests: None,
        installed_size: None,
        sandbox: None,
        completions: None
    }
}

/// Returns an alternatives entry.
fn entry(
    name: &str,
    path: &str,
    link: &str,
    priority: i32
) -> AlternativeEntry {
    AlternativeEntry {
        name: name.to_string(),
        path: path.to_string(),
        link: link.to_string(),
        priority
    }
}

/// Writes a manifest into the target's package store, mirroring the layout
/// `zoi-resolver` produces: `<store>/<hash>-<name>/<version>/manifest.json`.
fn install_into_store(root: &Path, manifest: &InstallManifest) {
    let dir = root
        .join("var/lib/zoi/pkgs/store")
        .join(format!(
            "0123456789abcdef0123456789abcdef-{}",
            manifest.name
        ))
        .join(&manifest.version);
    fs::create_dir_all(&dir).expect("create store dir");
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(manifest).expect("serialize")
    )
    .expect("write manifest");
}

/// Removes a package's store directory, simulating an uninstall.
fn remove_from_store(root: &Path, name: &str) {
    let dir = root
        .join("var/lib/zoi/pkgs/store")
        .join(format!("0123456789abcdef0123456789abcdef-{name}"));
    let _ = fs::remove_dir_all(dir);
}

/// Creates an executable file inside the target so symlinks resolve.
fn make_binary(root: &Path, path: &str) {
    let full = root.join(path.trim_start_matches('/'));
    fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    fs::write(&full, "#!/bin/sh\n").expect("write");
}

fn read_link(root: &Path, path: &str) -> String {
    fs::read_link(root.join(path.trim_start_matches('/')))
        .expect("link to exist")
        .to_string_lossy()
        .to_string()
}

/// Reconciles inside `root` by pointing the global sysroot at it for the
/// duration of the test.
fn reconcile_in(root: &Path, touched: &[String]) -> Vec<String> {
    zoi::alternatives::reconcile_in(root, touched).expect("reconcile")
}

#[test]
fn manifest_in_the_store_drives_registration() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );

    let changed = reconcile_in(root, &["gawk".to_string()]);

    assert_eq!(changed, vec!["awk".to_string()]);
    assert!(read_link(root, "/usr/bin/awk").ends_with("/etc/alternatives/awk"));
    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
    );
}

#[test]
fn two_packages_coexist_and_the_higher_priority_wins() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    make_binary(root, "/usr/bin/mawk");

    install_into_store(
        root,
        &manifest_with(
            "mawk",
            vec![entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)]
        )
    );
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );

    reconcile_in(root, &["mawk".to_string(), "gawk".to_string()]);

    // Neither package fights the other over /usr/bin/awk; both register and the
    // higher priority is selected.
    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
    );

    let groups = zoi::alternatives::list_in(root).expect("list");
    assert_eq!(groups.len(), 1);
    let first = groups.first().expect("one group registered");
    assert_eq!(first.alternatives.len(), 2);
}

#[test]
fn removing_one_package_falls_back_to_the_other() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    make_binary(root, "/usr/bin/mawk");

    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );
    install_into_store(
        root,
        &manifest_with(
            "mawk",
            vec![entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)]
        )
    );
    reconcile_in(root, &["gawk".to_string(), "mawk".to_string()]);

    // Uninstall gawk.
    remove_from_store(root, "gawk");
    reconcile_in(root, &["gawk".to_string()]);

    // /usr/bin/awk must still work. This is the behaviour that makes having two
    // implementations of awk possible at all.
    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/mawk"),
        "expected a fallback to mawk"
    );
    assert!(read_link(root, "/usr/bin/awk").ends_with("/etc/alternatives/awk"));
}

#[test]
fn removing_the_last_package_removes_the_canonical_link() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );
    reconcile_in(root, &["gawk".to_string()]);

    remove_from_store(root, "gawk");
    reconcile_in(root, &["gawk".to_string()]);

    assert_eq!(
        zoi::alternatives::list_in(root).expect("list"),
        [] as [zoi::alternatives::AlternativeGroup; 0]
    );
    // A dangling /usr/bin/awk would break every caller in a confusing way.
    assert!(root.join("usr/bin/awk").symlink_metadata().is_err());
}

#[test]
fn a_manual_choice_survives_an_unrelated_install() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    make_binary(root, "/usr/bin/mawk");

    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );
    install_into_store(
        root,
        &manifest_with(
            "mawk",
            vec![entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)]
        )
    );
    reconcile_in(root, &["gawk".to_string(), "mawk".to_string()]);

    zoi::alternatives::set_in(root, "awk", "/usr/bin/mawk").expect("set");

    // Re-running reconciliation, as an upgrade would, must not undo the choice.
    reconcile_in(root, &["gawk".to_string()]);

    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/mawk"),
        "a manual selection must be preserved across reconciliation"
    );
}

#[test]
fn dropping_an_entry_from_a_definition_deactivates_it() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    make_binary(root, "/usr/bin/mawk");

    let combined = manifest_with(
        "awk-impls",
        vec![
            entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100),
            entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50),
        ]
    );
    install_into_store(root, &combined);
    reconcile_in(root, &["awk-impls".to_string()]);
    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/gawk")
    );

    // Reinstall with only mawk declared, as if the definition were edited.
    install_into_store(
        root,
        &manifest_with(
            "awk-impls",
            vec![entry("awk", "/usr/bin/mawk", "/usr/bin/awk", 50)]
        )
    );
    reconcile_in(root, &["awk-impls".to_string()]);

    // The removed entry must not stay active, which is what re-registration
    // alone would leave behind.
    assert!(
        read_link(root, "/etc/alternatives/awk").ends_with("/usr/bin/mawk")
    );

    let group = zoi::alternatives::get_in(root, "awk")
        .expect("get")
        .expect("group");
    assert_eq!(group.alternatives.len(), 1);
}

#[test]
fn reconcile_is_idempotent() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );

    let first = reconcile_in(root, &["gawk".to_string()]);
    let second = reconcile_in(root, &["gawk".to_string()]);

    assert_eq!(first, vec!["awk".to_string()]);
    assert!(
        second.is_empty(),
        "a second run must report no change: {second:?}"
    );

    let groups = zoi::alternatives::list_in(root).expect("list");
    let first = groups.first().expect("one group registered");
    assert_eq!(first.alternatives.len(), 1);
}

#[test]
fn a_package_without_alternatives_changes_nothing() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    install_into_store(root, &manifest_with("unrelated", vec![]));

    let changed = reconcile_in(root, &["unrelated".to_string()]);
    assert_eq!(changed, [] as [std::string::String; 0]);
    assert_eq!(
        zoi::alternatives::list_in(root).expect("list"),
        [] as [zoi::alternatives::AlternativeGroup; 0]
    );
}

#[test]
fn several_groups_from_one_package_are_all_registered() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    for bin in ["/usr/bin/gawk", "/usr/bin/vim", "/usr/bin/nvim"] {
        make_binary(root, bin);
    }

    install_into_store(
        root,
        &manifest_with(
            "gvim",
            vec![
                entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100),
                entry("editor", "/usr/bin/vim", "/usr/bin/editor", 100),
                entry("vi", "/usr/bin/vim", "/usr/bin/vi", 100),
            ]
        )
    );

    reconcile_in(root, &["gvim".to_string()]);

    let mut names: Vec<String> = zoi::alternatives::list_in(root)
        .expect("list")
        .into_iter()
        .map(|g| g.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["awk", "editor", "vi"]);

    assert!(read_link(root, "/usr/bin/vi").ends_with("/etc/alternatives/vi"));
    assert!(
        read_link(root, "/usr/bin/editor")
            .ends_with("/etc/alternatives/editor")
    );
}

#[test]
fn a_corrupt_manifest_does_not_panic() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    let dir = root
        .join("var/lib/zoi/pkgs/store")
        .join("0123456789abcdef0123456789abcdef-broken")
        .join("1.0.0");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("manifest.json"), "{ not json").expect("write");

    // Treated as "not installed", so this deregisters rather than registering
    // from garbage. The important property is that it does not panic.
    let _ = reconcile_in(root, &["broken".to_string()]);
}

#[test]
fn a_missing_manifest_is_treated_as_not_installed() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );
    reconcile_in(root, &["gawk".to_string()]);

    // The manifest disappears without the index being updated, as can happen
    // after a crash. The alternative must not be left registered.
    let dir = root
        .join("var/lib/zoi/pkgs/store")
        .join("0123456789abcdef0123456789abcdef-gawk");
    fs::remove_dir_all(&dir).expect("remove");

    reconcile_in(root, &["gawk".to_string()]);
    assert_eq!(
        zoi::alternatives::list_in(root).expect("list"),
        [] as [zoi::alternatives::AlternativeGroup; 0]
    );
}

#[test]
fn user_scope_packages_are_found_too() {
    // A user-scope package registering an alternative under the user's own PATH
    // is legitimate, so the user store must be searched.
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    // Inside a sysroot the user store is the XDG default under that root.
    let user_store = root.join(".local/share/zoi/pkgs/store");
    let dir = user_store
        .join("0123456789abcdef0123456789abcdef-uawk")
        .join("1.0.0");
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest_with(
            "uawk",
            vec![entry("uawk", "/usr/bin/uawk", "/usr/bin/uawk-sh", 100)]
        ))
        .expect("serialize")
    )
    .expect("write");

    let changed = zoi::alternatives::reconcile_in(root, &["uawk".to_string()])
        .expect("reconcile");
    assert_eq!(changed, vec!["uawk".to_string()]);

    let _ = fs::remove_dir_all(
        user_store.join("0123456789abcdef0123456789abcdef-uawk")
    );
}

#[test]
fn state_directory_is_not_inside_the_package_store() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();

    make_binary(root, "/usr/bin/gawk");
    install_into_store(
        root,
        &manifest_with(
            "gawk",
            vec![entry("awk", "/usr/bin/gawk", "/usr/bin/awk", 100)]
        )
    );
    reconcile_in(root, &["gawk".to_string()]);

    // If state lived under the store it would be destroyed by the very
    // uninstall that retention has to survive.
    let state = root.join("var/lib/zoi/alternatives/awk/state.json");
    assert!(state.is_file());
    assert!(!state.starts_with(root.join("var/lib/zoi/pkgs/store")));
}

#[test]
fn alternative_entry_defaults_to_priority_100() {
    // A package that omits `priority` gets Debian's default, so declarations
    // stay short and behave predictably.
    let entry: AlternativeEntry = serde_json::from_str(
        r#"{"name":"awk","path":"/usr/bin/gawk","link":"/usr/bin/awk"}"#
    )
    .expect("parse");
    assert_eq!(entry.priority, 100);
}

#[test]
fn lua_metadata_parses_the_alternatives_block() {
    // The field has to survive the trip from a .pkg.lua metadata block into the
    // Package struct, or packages cannot declare alternatives at all.
    let pkg: zoi::pkg::types::Package = serde_json::from_str(
        r#"{
            "name": "gawk",
            "repo": "core",
            "version": "5.2.2",
            "description": "The GNU awk",
            "maintainer": {"name": "Test", "email": "t@example.invalid"},
            "alternatives": [
                {"name":"awk","path":"/usr/bin/gawk","link":"/usr/bin/awk","priority":100}
            ]
        }"#,
    )
    .expect("parse package");

    let alternatives = pkg.alternatives.expect("alternatives");
    assert_eq!(alternatives.len(), 1);
    let first = alternatives.first().expect("one alternative");
    assert_eq!(first.name, "awk");
    assert_eq!(first.priority, 100);
}

#[test]
fn a_package_without_the_field_deserialises_cleanly() {
    // Existing manifests and definitions have no `alternatives` key, and must
    // keep working.
    let pkg: zoi::pkg::types::Package = serde_json::from_str(
        r#"{"name":"gawk","repo":"core","version":"5.2.2","description":"x",
            "maintainer":{"name":"Test","email":"t@example.invalid"}}"#
    )
    .expect("parse package");
    assert!(pkg.alternatives.is_none());

    let manifest = manifest_with("gawk", vec![]);
    assert!(manifest.alternatives.is_none());
}
