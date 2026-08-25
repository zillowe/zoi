//! Tests for general Zoi utility functions.

use zoi::pkg::utils;

mod common;

#[test]
fn user_paths_should_prefer_xdg_directories() {
    let mut ctx = common::TestContextGuard::acquire();
    let root = tempfile::tempdir().expect("temporary root should exist");
    let config = root.path().join("config");
    let data = root.path().join("data");
    let cache = root.path().join("cache");
    let state = root.path().join("state");
    ctx.set_env_var("XDG_CONFIG_HOME", &config);
    ctx.set_env_var("XDG_DATA_HOME", &data);
    ctx.set_env_var("XDG_CACHE_HOME", &cache);
    ctx.set_env_var("XDG_STATE_HOME", &state);

    assert_eq!(
        utils::get_user_config_dir().expect("config directory should resolve"),
        config.join("zoi")
    );
    assert_eq!(
        utils::get_user_data_dir().expect("data directory should resolve"),
        data.join("zoi")
    );
    assert_eq!(
        utils::get_user_cache_dir().expect("cache directory should resolve"),
        cache.join("zoi")
    );
    assert_eq!(
        utils::get_user_state_dir().expect("state directory should resolve"),
        state.join("zoi")
    );
    assert_eq!(
        utils::get_store_base_dir(zoi::pkg::types::Scope::User)
            .expect("user store should resolve"),
        data.join("zoi/pkgs/store")
    );
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[test]
fn relative_xdg_paths_should_fall_back_to_the_standard_unix_locations() {
    let mut ctx = common::TestContextGuard::acquire();
    let home = tempfile::tempdir().expect("temporary home should exist");
    ctx.set_env_var("HOME", home.path());
    ctx.set_env_var("XDG_DATA_HOME", "relative-data");

    assert_eq!(
        utils::get_user_data_dir().expect("data directory should resolve"),
        home.path().join(".local/share/zoi")
    );
}

#[test]
fn project_paths_should_remain_in_the_project_zoi_directory() {
    let mut ctx = common::TestContextGuard::acquire();
    let project = tempfile::tempdir().expect("temporary project should exist");
    ctx.set_current_dir(project.path());

    assert_eq!(
        utils::get_store_base_dir(zoi::pkg::types::Scope::Project)
            .expect("project store should resolve"),
        project.path().join(".zoi/pkgs/store")
    );
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[test]
fn system_paths_should_follow_fhs_on_unix() {
    assert_eq!(
        utils::get_system_config_dir(),
        std::path::PathBuf::from("/etc/zoi")
    );
    assert_eq!(
        utils::get_system_data_dir(),
        std::path::PathBuf::from("/var/lib/zoi")
    );
    assert_eq!(
        utils::get_system_cache_dir(),
        std::path::PathBuf::from("/var/cache/zoi")
    );
}

#[test]
fn test_generate_package_id() {
    let id1 = utils::generate_package_id("zoidberg", "core", "hello");
    let id2 = utils::generate_package_id("zoidberg", "core", "hello");
    let id3 = utils::generate_package_id("zoidberg", "community", "hello");

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
    assert_eq!(id1.len(), 32);
}

#[test]
fn test_get_package_dir_name() {
    let id = "abc123def4567890abc123def4567890";
    let dir_name = utils::get_package_dir_name(id, "hello");
    assert_eq!(dir_name, "abc123def4567890abc123def4567890-hello");
}

#[test]
fn get_filename_from_url_should_exclude_query_and_fragment() {
    assert_eq!(
        zoi::pkg::install::util::get_filename_from_url(
            "https://example.test/releases/zoi-1.0.zpa?token=value#ignored"
        ),
        "zoi-1.0.zpa"
    );
}

#[test]
fn test_is_safe_path() {
    use std::path::Path;
    let base = Path::new("/tmp/zoi-staging");

    // Safe paths
    assert!(zoi::utils::is_safe_path(base, Path::new("bin/hello")));
    assert!(zoi::utils::is_safe_path(base, Path::new("./bin/hello")));
    assert!(zoi::utils::is_safe_path(
        base,
        Path::new("usr/lib/libtest.so")
    ));

    // Dangerous paths (traversal)
    assert!(!zoi::utils::is_safe_path(base, Path::new("../etc/shadow")));
    assert!(!zoi::utils::is_safe_path(
        base,
        Path::new("bin/../../etc/shadow")
    ));
    assert!(!zoi::utils::is_safe_path(base, Path::new("/etc/shadow")));
}

#[cfg(not(windows))]
#[test]
fn command_exists_should_not_interpret_shell_syntax() {
    assert!(!zoi::utils::command_exists(
        "definitely-not-a-zoi-command; true"
    ));
}

#[cfg(unix)]
#[test]
fn copy_dir_all_should_preserve_symbolic_links() {
    use std::fs;

    let source = tempfile::tempdir().expect("source directory should exist");
    let destination =
        tempfile::tempdir().expect("destination directory should exist");
    fs::write(source.path().join("target"), "zoi")
        .expect("target should be written");
    std::os::unix::fs::symlink("target", source.path().join("link"))
        .expect("link should be created");

    utils::copy_dir_all(source.path(), destination.path())
        .expect("directory should be copied");

    assert!(destination.path().join("link").is_symlink());
    assert_eq!(
        fs::read_link(destination.path().join("link"))
            .expect("link target should be readable"),
        std::path::Path::new("target")
    );
}
