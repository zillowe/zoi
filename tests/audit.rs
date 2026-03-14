use chrono::Utc;
use zoi::pkg::audit::{AuditAction, AuditEntry};
use zoi::pkg::types::{InstallManifest, InstallReason, PackageType, Scope};

#[test]
fn test_audit_entry_serialization() {
    let manifest = InstallManifest {
        name: "audit-test".to_string(),
        version: "2.0.0".to_string(),
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
    };

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
