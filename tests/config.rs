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
