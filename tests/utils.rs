use zoi::pkg::utils;

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
