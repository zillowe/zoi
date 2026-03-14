use zoi::pkg::transaction;
use zoi::pkg::types::{InstallManifest, InstallReason, PackageType, Scope, TransactionOperation};

#[test]
fn test_transaction_lifecycle() {
    let transaction = transaction::begin().unwrap();
    let id = transaction.id.clone();

    let manifest = InstallManifest {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        sub_package: None,
        repo: "test".to_string(),
        registry_handle: "local".to_string(),
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
        install_method: Some("test".to_string()),
        service: None,
        installed_files: vec!["/tmp/zoi-test-file".to_string()],
        installed_size: None,
    };

    transaction::record_operation(
        &id,
        TransactionOperation::Install {
            manifest: Box::new(manifest),
        },
    )
    .unwrap();

    let modified = transaction::get_modified_files(&id).unwrap();
    assert_eq!(modified.len(), 1);
    assert_eq!(modified[0], "/tmp/zoi-test-file");

    transaction::commit(&id).unwrap();
}
