use chrono::Utc;
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;
use zoi::pkg::audit::{self, AuditAction, AuditEntry};
use zoi::pkg::types::{InstallManifest, InstallReason, PackageType, Scope};
use zoi::pkg::{config, types};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_manifest(name: &str, version: &str) -> InstallManifest {
    InstallManifest {
        name: name.to_string(),
        version: version.to_string(),
        sub_package: None,
        repo: "community".to_string(),
        registry_handle: "zoidberg".to_string(),
        package_type: PackageType::Package,
        reason: InstallReason::Direct,
        scope: Scope::User,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: Some("pre-compiled".to_string()),
        service: None,
        installed_files: vec![],
        installed_size: None,
    }
}

#[test]
fn test_audit_entry_serialization() {
    let manifest = test_manifest("audit-test", "2.0.0");

    let entry = AuditEntry {
        timestamp: Utc::now(),
        user: "test-user".to_string(),
        action: AuditAction::Install,
        package_name: manifest.name.clone(),
        version: manifest.version.clone(),
        repo: manifest.repo.clone(),
        package_type: manifest.package_type,
        scope: manifest.scope,
        registry: manifest.registry_handle.clone(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("audit-test"));
    assert!(json.contains("test-user"));
    assert!(json.contains("Install"));
}

#[test]
fn test_audit_hash_chain_verification_and_tamper_detection() {
    let _guard = ENV_LOCK.lock().expect("env lock should be available");
    let tmp = tempdir().expect("tempdir should be created");
    let old_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", tmp.path());
    }

    let cfg = types::Config {
        audit_log_enabled: true,
        ..Default::default()
    };
    config::write_user_config(&cfg).expect("config should be written");

    let manifest_a = test_manifest("audit-a", "1.0.0");
    let manifest_b = test_manifest("audit-b", "1.1.0");

    audit::log_event(AuditAction::Install, &manifest_a).expect("audit entry A should be logged");
    audit::log_event(AuditAction::Upgrade, &manifest_b).expect("audit entry B should be logged");

    let report = audit::verify_chain().expect("audit chain should be verifiable");
    assert!(report.valid, "expected valid chain: {}", report.message);
    assert_eq!(report.total_entries, 2);
    assert_eq!(report.hashed_entries, 2);

    let log_path = tmp.path().join(".zoi").join("audit.json");
    let content = fs::read_to_string(&log_path).expect("audit log should exist");
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut tampered: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("first audit line should be valid JSON");
    tampered["package_name"] = serde_json::Value::String("tampered-package".to_string());
    lines[0] = serde_json::to_string(&tampered).expect("tampered line should serialize");
    fs::write(&log_path, format!("{}\n", lines.join("\n"))).expect("tampered log should write");

    let tamper_report = audit::verify_chain().expect("tampered chain should still parse");
    assert!(!tamper_report.valid);
    assert!(
        tamper_report.message.contains("hash mismatch"),
        "unexpected tamper message: {}",
        tamper_report.message
    );

    if let Some(old) = old_home {
        unsafe {
            std::env::set_var("HOME", old);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }
}
