//! Integration tests for hashing, PGP signatures, and security verification.

use std::fs;
use tempfile::tempdir;
use zoi::pkg::{hash, helper, pgp};

#[test]
fn test_hash_verification() {
    let dir = tempdir().expect("unwrap failed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "zoi-test-content").expect("unwrap failed");

    let calculated =
        helper::get_hash(file_path.to_str().expect("unwrap failed"), helper::HashType::Sha512).expect("unwrap failed");
    let second_run =
        helper::get_hash(file_path.to_str().expect("unwrap failed"), helper::HashType::Sha512).expect("unwrap failed");
    assert_eq!(calculated, second_run);
    assert_eq!(calculated.len(), 128);

    let calculated_sha256 =
        helper::get_hash(file_path.to_str().expect("unwrap failed"), helper::HashType::Sha256).expect("unwrap failed");
    assert_eq!(calculated_sha256.len(), 64);
}

#[test]
fn test_directory_hashing() {
    let dir = tempdir().expect("unwrap failed");
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).expect("unwrap failed");

    fs::write(dir.path().join("a.txt"), "content-a").expect("unwrap failed");
    fs::write(sub.join("b.txt"), "content-b").expect("unwrap failed");

    let hash1 = hash::calculate_dir_hash(dir.path()).expect("unwrap failed");
    let hash2 = hash::calculate_dir_hash(dir.path()).expect("unwrap failed");

    assert_eq!(hash1, hash2, "Directory hashing must be deterministic");

    fs::write(sub.join("b.txt"), "content-b-changed").expect("unwrap failed");
    let hash3 = hash::calculate_dir_hash(dir.path()).expect("unwrap failed");
    assert_ne!(hash1, hash3, "Hash must change when content changes");
}

#[test]
fn test_builtin_pgp_loading() {
    let res = pgp::ensure_builtin_keys();
    assert!(res.is_ok());
}
