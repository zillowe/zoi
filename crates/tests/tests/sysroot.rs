//! Integration tests for sysroot management and isolation.

use std::path::PathBuf;

use zoi::pkg::sysroot;

mod common;

#[test]
fn test_sysroot_functionality() {
    let _ctx = common::TestContextGuard::acquire();
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/new-disk"));

    let path_abs = PathBuf::from("/etc/zoi/config.yaml");
    let applied_abs = sysroot::apply_sysroot(path_abs);
    assert_eq!(
        applied_abs,
        PathBuf::from("/mnt/new-disk/etc/zoi/config.yaml")
    );

    let path_rel = PathBuf::from("usr/bin/git");
    let applied_rel = sysroot::apply_sysroot(path_rel);
    assert_eq!(applied_rel, PathBuf::from("/mnt/new-disk/usr/bin/git"));
}

#[test]
fn test_sysroot_can_be_replaced() {
    let _ctx = common::TestContextGuard::acquire();
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/first"));
    assert_eq!(
        sysroot::apply_sysroot(PathBuf::from("/etc/hosts")),
        PathBuf::from("/mnt/first/etc/hosts")
    );
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/second"));
    assert_eq!(
        sysroot::apply_sysroot(PathBuf::from("/etc/hosts")),
        PathBuf::from("/mnt/second/etc/hosts")
    );
}

#[test]
fn test_sysroot_should_contain_parent_traversal() {
    let _ctx = common::TestContextGuard::acquire();
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/root"));

    assert_eq!(
        sysroot::apply_sysroot(PathBuf::from("../../etc/passwd")),
        PathBuf::from("/mnt/root/etc/passwd")
    );
}

#[test]
fn host_db_root_ignores_the_sysroot() {
    // `zoi system distro build` bootstraps an empty target from the host's
    // synced registry. With the sysroot applied to it, the "host" database
    // was the target's own empty one: "No synced registries found".
    let mut ctx = common::TestContextGuard::acquire();
    let home = tempfile::tempdir().expect("tempdir");
    ctx.set_env_var("HOME", home.path());
    ctx.set_env_var("XDG_DATA_HOME", home.path().join(".local/share"));
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/new-disk"));

    let host = zoi::pkg::resolve::get_host_db_root().expect("host db root");
    assert_eq!(host, home.path().join(".local/share/zoi/pkgs/db"));
}

#[test]
fn archive_cache_stays_on_the_host_while_building_into_a_sysroot() {
    // Downloads are kept outside the target, so a failed or repeated
    // `zoi system distro build` reuses them instead of downloading again.
    let mut ctx = common::TestContextGuard::acquire();
    let home = tempfile::tempdir().expect("tempdir");
    ctx.set_env_var("HOME", home.path());
    ctx.set_env_var("XDG_CACHE_HOME", home.path().join(".cache"));
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/new-disk"));

    let cache = zoi::pkg::cache::get_archive_cache_root().expect("cache");
    assert!(!cache.starts_with("/mnt/new-disk"), "{}", cache.display());
    if !zoi_core::utils::is_effective_root() {
        assert_eq!(cache, home.path().join(".cache/zoi/archives"));
    }
}

#[test]
fn sync_and_resolution_use_the_same_registry_location_on_a_sysroot() {
    // `zoi --root <dir> sync` writes the system-scope databases inside the
    // target. Resolution used to read the user-scope ones, so `update` and
    // `install` never saw what the sync fetched.
    let mut ctx = common::TestContextGuard::acquire();
    let home = tempfile::tempdir().expect("tempdir");
    ctx.set_env_var("HOME", home.path());
    ctx.set_env_var("XDG_DATA_HOME", home.path().join(".local/share"));
    ctx.remove_env_var("ZOI_DB_DIR");
    // No project `.zoi` database may be picked up instead.
    ctx.set_current_dir(home.path());
    common::TestContextGuard::set_sysroot(PathBuf::from("/mnt/new-disk"));

    assert_eq!(
        zoi_core::utils::default_db_scope(),
        zoi_core::types::Scope::System
    );
    let system = zoi_core::utils::get_db_base_dir(
        zoi_core::types::Scope::System
    )
    .expect("system db dir");
    assert!(system.starts_with("/mnt/new-disk"), "{}", system.display());
    assert_eq!(zoi::pkg::resolve::get_db_root().expect("db root"), system);
    assert_eq!(zoi_core::utils::get_db_root().expect("db root"), system);
}
