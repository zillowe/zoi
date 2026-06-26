use std::path::PathBuf;
use zoi::pkg::sysroot;

mod common;

#[test]
fn test_sysroot_functionality() {
    let ctx = common::TestContextGuard::acquire();
    ctx.set_sysroot(PathBuf::from("/mnt/new-disk"));

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
    let ctx = common::TestContextGuard::acquire();
    ctx.set_sysroot(PathBuf::from("/mnt/first"));
    assert_eq!(
        sysroot::apply_sysroot(PathBuf::from("/etc/hosts")),
        PathBuf::from("/mnt/first/etc/hosts")
    );
    ctx.set_sysroot(PathBuf::from("/mnt/second"));
    assert_eq!(
        sysroot::apply_sysroot(PathBuf::from("/etc/hosts")),
        PathBuf::from("/mnt/second/etc/hosts")
    );
}
