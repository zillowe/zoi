use tempfile::tempdir;
use zoi::pkg::{config, telemetry, types};

mod common;

#[test]
fn test_telemetry_respects_opt_in() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pkg = types::Package {
        name: "test-pkg".to_string(),
        ..Default::default()
    };

    let res = telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None).unwrap();
    assert!(!res, "Telemetry should return false when disabled");

    let cfg = types::Config {
        telemetry_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).unwrap();

    let res_enabled = telemetry::posthog_capture_event("test", &pkg, "1.0.0", "local", None);

    if let Ok(sent) = res_enabled {
        assert!(sent, "Telemetry should have attempted to send when enabled")
    }
}
