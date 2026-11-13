//! Security regression tests for global hooks when running as root.
//!
//! When Zoi runs with an effective UID of 0 (e.g. under `sudo`), global hooks
//! must only come from root-controlled locations, every hook file must be
//! owned by root and not group/world-writable, and the trust database must
//! live in a system location.

use std::fs;

use tempfile::tempdir;
use zoi_hooks::global::{
    get_system_hooks_dir, get_trusted_hashes_path, get_user_hooks_dir,
    load_hooks_for_privilege
};

mod common;

/// Writes a minimal hook definition named `name`.
fn hook_yaml(name: &str) -> String {
    format!(
        "name: {name}\ndescription: test\ntrigger:\n  paths: \
         [\"*\"]\naction:\n  when: PostTransaction\n  exec: echo {name}\n"
    )
}

#[test]
fn root_ignores_user_and_untrusted_hook_files() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let home = root.join("home");
    fs::create_dir_all(&home).expect("home");
    ctx.set_env_var("HOME", &home);
    ctx.set_env_var("XDG_DATA_HOME", home.join(".local/share"));
    common::TestContextGuard::set_sysroot(root.clone());

    let user_hooks = get_user_hooks_dir().expect("user hooks dir");
    let system_hooks = get_system_hooks_dir().expect("system hooks dir");
    fs::create_dir_all(&system_hooks).expect("system hooks dir");
    fs::write(
        user_hooks.join("user.hook.yaml"),
        hook_yaml("fixsec-user-hook")
    )
    .expect("user hook");
    fs::write(
        system_hooks.join("system.hook.yaml"),
        hook_yaml("fixsec-system-hook")
    )
    .expect("system hook");

    // Unprivileged behavior is unchanged: both hooks are visible.
    let names: Vec<String> = load_hooks_for_privilege(false)
        .expect("hooks load")
        .into_iter()
        .map(|h| h.name)
        .collect();
    assert!(names.contains(&"fixsec-user-hook".to_string()));
    assert!(names.contains(&"fixsec-system-hook".to_string()));

    // As root: the user directory is never consulted, and the system hook is
    // skipped because the test user (not root) owns it.
    let root_hooks = load_hooks_for_privilege(true).expect("hooks load");
    let root_names: Vec<String> =
        root_hooks.iter().map(|h| h.name.clone()).collect();
    assert!(!root_names.contains(&"fixsec-user-hook".to_string()));
    if cfg!(unix) && !zoi_core::utils::is_effective_root() {
        assert!(!root_names.contains(&"fixsec-system-hook".to_string()));
    }
    assert!(
        root_hooks.iter().any(|h| h.is_builtin),
        "builtin hooks are still available"
    );
}

#[test]
fn root_trust_db_lives_in_the_system_data_dir() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).expect("home");
    ctx.set_env_var("HOME", &home);
    ctx.set_env_var("XDG_DATA_HOME", home.join(".local/share"));

    let user_db = get_trusted_hashes_path(false).expect("user trust db");
    let root_db = get_trusted_hashes_path(true).expect("root trust db");
    assert!(user_db.starts_with(&home));
    assert!(!root_db.starts_with(&home));
    assert!(
        root_db.starts_with(zoi_core::utils::get_system_data_dir()),
        "root trust db should be under the system data dir: {}",
        root_db.display()
    );
}

#[cfg(unix)]
#[test]
fn ownership_check_rejects_files_not_owned_by_root() {
    if zoi_core::utils::is_effective_root() {
        return;
    }
    let tmp = tempdir().expect("tempdir");
    let file = tmp.path().join("hook.yaml");
    fs::write(&file, "x").expect("file");
    assert!(!zoi_core::utils::is_root_owned_and_protected(&file));
    assert!(!zoi_core::utils::is_root_owned_and_protected(
        &tmp.path().join("missing")
    ));
}
