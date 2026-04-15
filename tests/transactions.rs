use std::fs;
use tempfile::tempdir;
use zoi::pkg::transaction;
use zoi::pkg::types::{InstallManifest, InstallReason, PackageType, Scope, TransactionOperation};

mod common;

fn sample_manifest(name: &str, files: Vec<&str>) -> InstallManifest {
    InstallManifest {
        name: name.to_string(),
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
        installed_files: files.into_iter().map(str::to_string).collect(),
        installed_size: None,
    }
}

#[test]
fn test_transaction_lifecycle() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().unwrap();
    ctx.set_env_var("HOME", tmp.path());

    let transaction = transaction::begin().unwrap();
    let id = transaction.id.clone();
    let transaction_path = tmp
        .path()
        .join(".zoi/transactions")
        .join(format!("{}.json", id));

    transaction::record_operation(
        &id,
        TransactionOperation::Install {
            manifest: Box::new(sample_manifest("test-pkg", vec!["/tmp/zoi-test-file"])),
        },
    )
    .unwrap();

    assert!(
        transaction_path.exists(),
        "transaction log should exist before commit"
    );

    let modified = transaction::get_modified_files(&id).unwrap();
    assert_eq!(modified.len(), 1);
    assert_eq!(modified[0], "/tmp/zoi-test-file");

    transaction::commit(&id).unwrap();
    assert!(
        !transaction_path.exists(),
        "commit should remove the transaction log"
    );
}

#[test]
fn test_transaction_get_modified_files_deduplicates_upgrade_paths() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().unwrap();
    ctx.set_env_var("HOME", tmp.path());

    let transaction = transaction::begin().unwrap();
    let id = transaction.id.clone();

    transaction::record_operation(
        &id,
        TransactionOperation::Upgrade {
            old_manifest: Box::new(sample_manifest("test-pkg", vec!["/tmp/shared", "/tmp/old"])),
            new_manifest: Box::new(sample_manifest("test-pkg", vec!["/tmp/shared", "/tmp/new"])),
        },
    )
    .unwrap();

    let mut modified = transaction::get_modified_files(&id).unwrap();
    modified.sort();
    assert_eq!(
        modified,
        vec![
            "/tmp/new".to_string(),
            "/tmp/old".to_string(),
            "/tmp/shared".to_string()
        ]
    );
}

#[test]
fn test_transaction_begin_writes_log_file() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().unwrap();
    ctx.set_env_var("HOME", tmp.path());

    let transaction = transaction::begin().unwrap();
    let transaction_path = tmp
        .path()
        .join(".zoi/transactions")
        .join(format!("{}.json", transaction.id));

    assert!(
        transaction_path.exists(),
        "begin should create a transaction log"
    );
    let content = fs::read_to_string(transaction_path).unwrap();
    assert!(content.contains(&transaction.id));
}
