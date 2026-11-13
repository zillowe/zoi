//! Package hooks never act on the host while installing into a sysroot.

use tempfile::tempdir;
use zoi::pkg::hooks::{HookType, run_hooks};
use zoi::pkg::types::{Hooks, PlatformOrStringVec, Scope};

mod common;

#[test]
#[cfg(target_os = "linux")]
fn package_hooks_do_not_run_on_the_host_inside_a_sysroot() {
    let _ctx = common::TestContextGuard::acquire();
    let host = tempdir().expect("tempdir");
    let target = tempdir().expect("tempdir");
    let marker = host.path().join("touched-by-hook");

    // The target has no shell yet, so the hook is skipped rather than run
    // with the host's bash (where it used to edit the host's /etc/shells).
    common::TestContextGuard::set_sysroot(target.path().to_path_buf());
    let hooks = Hooks {
        post_install: Some(PlatformOrStringVec::StringVec(vec![format!(
            "touch '{}'",
            marker.display()
        )])),
        ..Default::default()
    };
    run_hooks(&hooks, HookType::PostInstall, Scope::System)
        .expect("hook is skipped, not failed");

    assert!(!marker.exists(), "hook ran on the host");
}
