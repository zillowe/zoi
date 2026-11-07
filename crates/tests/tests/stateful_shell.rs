//! Integration tests for stateful shell environments and Lua integration.

use std::fs;

use mlua::Lua;
use tempfile::tempdir;
use zoi::pkg::lua::functions;

mod common;

#[test]
fn test_cmd_is_stateful_across_calls() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let build_dir = root.join("build");
    fs::create_dir(&build_dir).expect("unwrap failed");

    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        Some(build_dir.to_str().expect("unwrap failed")),
        None,
        None,
        None,
        None,
        true
    )
    .expect("unwrap failed");

    // Test directory persistence
    let sub_dir = build_dir.join("stateful_test");
    fs::create_dir(&sub_dir).expect("unwrap failed");

    lua.load(r#"cmd("cd stateful_test")"#)
        .exec()
        .expect("unwrap failed");

    let (stdout, _, _) = lua
        .load(r#"return cmd("pwd")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");

    // Normalize paths for comparison (especially on Windows)
    let actual_pwd = std::path::PathBuf::from(stdout.trim());
    assert!(
        actual_pwd.ends_with("stateful_test"),
        "PWD should persist across cmd() calls. Got: {actual_pwd:?}"
    );

    // Test environment variable persistence
    lua.load(r#"cmd("export ZOI_STATE_TEST=persistent_value")"#)
        .exec()
        .expect("unwrap failed");

    let (stdout_env, _, _) = lua
        .load(r#"return cmd("echo $ZOI_STATE_TEST")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert_eq!(
        stdout_env.trim(),
        "persistent_value",
        "Environment variables should persist across cmd() calls"
    );
}

#[test]
fn test_cmd_handles_stderr_independently() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let build_dir = root.join("build_stderr");
    fs::create_dir(&build_dir).expect("unwrap failed");

    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        Some(build_dir.to_str().expect("unwrap failed")),
        None,
        None,
        None,
        None,
        true
    )
    .expect("unwrap failed");

    // Call 1: Success, no stderr
    let (_, stderr1, _) = lua
        .load(r#"return cmd("echo hello")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert!(
        stderr1.is_empty(),
        "Successful command should have empty stderr"
    );

    // Call 2: Failure, has stderr
    let (_, stderr2, exit_code2) = lua
        .load(r#"return cmd("ls /nonexistent_path_018e6e5a2e6b")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert!(
        !stderr2.is_empty(),
        "Failing command should have non-empty stderr"
    );
    assert_ne!(
        exit_code2, 0,
        "Failing command should have non-zero exit code"
    );

    // Call 3: Success again, stderr should be cleared/empty
    let (_, stderr3, _) = lua
        .load(r#"return cmd("echo world")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert!(
        stderr3.is_empty(),
        "Subsequent successful command should have empty stderr again. Got: \
         {stderr3:?}"
    );
}

#[test]
fn test_shell_recovers_if_killed() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let build_dir = root.join("build_recovery");
    fs::create_dir(&build_dir).expect("unwrap failed");

    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        Some(build_dir.to_str().expect("unwrap failed")),
        None,
        None,
        None,
        None,
        true
    )
    .expect("unwrap failed");

    // Call 1: Works
    lua.load(r#"cmd("echo alive")"#)
        .exec()
        .expect("unwrap failed");

    // Call 2: Kill the shell
    // We expect this to return an error because the sentinel will never be
    // printed as the shell dies
    let _ = lua.load(r#"cmd("exit 0")"#).exec();

    // Call 3: Should auto-respawn and work again
    let (stdout, _, _) = lua
        .load(r#"return cmd("echo back_from_the_dead")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert_eq!(
        stdout.trim(),
        "back_from_the_dead",
        "Shell should auto-respawn if killed"
    );
}

#[test]
fn test_cmd_quiet_options_and_dynamic_toggle() {
    let _ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let build_dir = root.join("build_quiet");
    fs::create_dir(&build_dir).expect("unwrap failed");

    let lua = Lua::new();
    functions::setup_lua_environment(
        &lua,
        "linux-amd64",
        None,
        None,
        None,
        Some(build_dir.to_str().expect("unwrap failed")),
        None,
        None,
        None,
        None,
        false // Start not quiet
    )
    .expect("unwrap failed");

    // Dynamic set_quiet in Lua
    lua.load(r"set_quiet(true)")
        .exec()
        .expect("set_quiet(true) should succeed");

    let (stdout, _, _) = lua
        .load(r#"return cmd("echo silenced")"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert_eq!(stdout.trim(), "silenced");

    // Override per-command with table option or boolean
    let (stdout2, _, _) = lua
        .load(r#"return cmd("echo override_table", { quiet = true })"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert_eq!(stdout2.trim(), "override_table");

    let (stdout3, _, _) = lua
        .load(r#"return cmd("echo override_bool", false)"#)
        .eval::<(String, String, i32)>()
        .expect("unwrap failed");
    assert_eq!(stdout3.trim(), "override_bool");

    // Dynamic set_quiet to false
    lua.load(r"set_quiet(false)")
        .exec()
        .expect("set_quiet(false) should succeed");
}

#[test]
fn test_package_metadata_quiet_stage_resolution() {
    use std::collections::HashMap;

    use zoi_core::types::{Package, QuietOption};

    // 1. None
    let pkg_default = Package::default();
    assert!(!pkg_default.is_stage_quiet("build", false));
    assert!(pkg_default.is_stage_quiet("build", true));

    // 2. Boolean true
    let pkg_bool = Package {
        quiet: Some(QuietOption::Boolean(true)),
        ..Default::default()
    };
    assert!(pkg_bool.is_stage_quiet("build", false));
    assert!(pkg_bool.is_stage_quiet("prepare", false));
    assert!(pkg_bool.is_stage_quiet("package", false));

    // 3. List
    let pkg_list = Package {
        quiet: Some(QuietOption::List(vec!["build".into(), "prepare".into()])),
        ..Default::default()
    };
    assert!(pkg_list.is_stage_quiet("build", false));
    assert!(pkg_list.is_stage_quiet("prepare", false));
    assert!(!pkg_list.is_stage_quiet("package", false));

    // 4. Map
    let mut map = HashMap::new();
    map.insert("build".to_string(), true);
    map.insert("prepare".to_string(), false);
    let pkg_map = Package {
        quiet: Some(QuietOption::Map(map)),
        ..Default::default()
    };
    assert!(pkg_map.is_stage_quiet("build", false));
    assert!(!pkg_map.is_stage_quiet("prepare", false));
    assert!(!pkg_map.is_stage_quiet("package", false));
}
