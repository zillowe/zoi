use zoi::pkg::config;
use zoi::pkg::types::Config;

#[test]
fn test_config_default_values() {
    let cfg = Config::default();
    assert!(cfg.rollback_enabled);
    assert!(!cfg.telemetry_enabled);
    assert_eq!(cfg.parallel_jobs, None);
}

#[test]
fn test_get_builtin_authorities() {
    let auths = config::get_builtin_authorities();
    assert!(auths.is_empty() || !auths[0].is_empty());
}
