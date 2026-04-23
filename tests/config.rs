mod common;

use tempfile::tempdir;
use zoi::pkg::config;
use zoi::pkg::types::{Config, Policy};

#[test]
fn test_config_default_values() {
    let cfg = Config::default();
    assert!(cfg.rollback_enabled);
    assert!(!cfg.telemetry_enabled);
    assert_eq!(cfg.parallel_jobs, None);
    assert!(!cfg.policy.parallel_jobs_unoverridable);
}

#[test]
fn test_get_builtin_authorities() {
    let auths = config::get_builtin_authorities();
    assert!(auths.is_empty() || !auths[0].is_empty());
}

#[test]
fn test_parallel_jobs_policy_field_deserializes() {
    let policy: Policy = serde_yaml::from_str(
        r#"
parallel_jobs_unoverridable: true
"#,
    )
    .expect("policy should deserialize");

    assert!(policy.parallel_jobs_unoverridable);
}

#[test]
fn test_cache_mirror_config_roundtrip_and_candidates() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir should be created");
    ctx.set_env_var("HOME", tmp.path());

    let first = "https://cache-1.example.com/zoi";
    let second = "https://cache-2.example.com/mirror";

    config::add_cache_mirror(first).expect("first mirror should be added");
    config::add_cache_mirror(second).expect("second mirror should be added");

    let cfg = config::read_config().expect("config should read");
    assert_eq!(
        cfg.cache_mirrors,
        vec![first.to_string(), second.to_string()]
    );

    let candidates =
        zoi::pkg::cache::mirror_candidate_urls("https://upstream.example.com/pkgs/foo.pkg.tar.zst");
    assert_eq!(
        candidates,
        vec![
            "https://upstream.example.com/pkgs/foo.pkg.tar.zst".to_string(),
            "https://cache-1.example.com/zoi/foo.pkg.tar.zst".to_string(),
            "https://cache-2.example.com/mirror/foo.pkg.tar.zst".to_string(),
        ]
    );

    config::remove_cache_mirror(first).expect("first mirror should be removed");
    let cfg = config::read_config().expect("config should read after removal");
    assert_eq!(cfg.cache_mirrors, vec![second.to_string()]);
}
