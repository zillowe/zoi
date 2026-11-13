//! Security regression tests for the Bubblewrap sandboxes.
//!
//! Covers: runtime-sandbox write paths restricted to the user's home and the
//! package's data directories, refusal to forward secret-looking environment
//! variables, and the restricted device/sysfs setup for sysroot builds.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi_sandbox::{
    RootProfile, is_sensitive_env_var, resolve_sandbox_write_path,
    wrap_command_in_root_with_profile
};

mod common;

#[test]
fn secret_looking_env_vars_are_never_forwarded() {
    for name in [
        "GPG_PASSWORD",
        "GITHUB_TOKEN",
        "NPM_TOKEN",
        "AWS_ACCESS_KEY_ID",
        "AWS_PROFILE",
        "MY_SECRET",
        "SIGNING_KEY",
        "DB_PASSWORD",
        "SSH_AUTH_SOCK",
        "github_token"
    ] {
        assert!(is_sensitive_env_var(name), "{name} should be blocked");
    }
    for name in [
        "LANG",
        "EDITOR",
        "MY_APP_MODE",
        "DISPLAY",
        "KEYBOARD_LAYOUT"
    ] {
        assert!(!is_sensitive_env_var(name), "{name} should be allowed");
    }
}

#[test]
fn write_paths_must_stay_inside_home_or_package_dirs() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let store = tmp.path().join("store").join("pkg");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&store).expect("store");
    ctx.set_env_var("HOME", &home);
    ctx.set_env_var("XDG_DATA_HOME", home.join(".local/share"));

    for ok in ["~/.config/app/", "~/.cache/app/state.db"] {
        assert!(
            resolve_sandbox_write_path(ok, &store).is_ok(),
            "{ok} should be allowed"
        );
    }
    let in_store = store.join("var");
    assert!(
        resolve_sandbox_write_path(in_store.to_str().expect("utf-8"), &store)
            .is_ok()
    );

    let outside = tmp.path().join("outside").join("file");
    for bad in [
        "/etc/zoi-fixsec-should-not-exist".to_string(),
        "~/../escape".to_string(),
        "~/".to_string(),
        "relative/path".to_string(),
        outside.display().to_string()
    ] {
        assert!(
            resolve_sandbox_write_path(&bad, &store).is_err(),
            "{bad} should be rejected"
        );
    }
    assert!(!Path::new("/etc/zoi-fixsec-should-not-exist").exists());
    assert!(!outside.exists());
}

#[test]
fn sysroot_builds_get_no_host_devices_and_read_only_sys() {
    if !zoi_core::utils::command_exists("bwrap") {
        println!("Skipping: bwrap not installed");
        return;
    }
    let tmp = tempdir().expect("tempdir");
    let env: HashMap<String, String> = HashMap::new();

    let args_of = |profile| -> Vec<String> {
        let cmd = wrap_command_in_root_with_profile(
            tmp.path(),
            Path::new("/bin/true"),
            &[],
            &env,
            &[],
            false,
            profile
        )
        .expect("command should be built");
        cmd.get_args()
            .map(OsStr::to_string_lossy)
            .map(std::borrow::Cow::into_owned)
            .collect()
    };

    let build = args_of(RootProfile::Build);
    assert!(!build.iter().any(|a| a == "--dev-bind"));
    assert!(
        build.windows(2).any(
            |w| matches!(w, [flag, dev] if flag == "--dev" && dev == "/dev")
        ),
        "{build:?}"
    );
    assert!(
        build.windows(3).any(|w| matches!(
            w,
            [flag, src, dst]
                if flag == "--ro-bind" && src == "/sys" && dst == "/sys"
        )),
        "{build:?}"
    );
    assert!(build.iter().any(|a| a == "--share-net"));

    // Hooks/exec/shell keep hardware access.
    let system = args_of(RootProfile::System);
    assert!(system.windows(3).any(|w| matches!(
        w,
        [flag, src, dst]
            if flag == "--dev-bind" && src == "/dev" && dst == "/dev"
    )));
}
