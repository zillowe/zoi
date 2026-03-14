use std::fs;
use tempfile::tempdir;
use zoi::pkg::{hash, helper, pgp};

#[test]
fn test_hash_verification() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "zoi-test-content").unwrap();

    let calculated =
        helper::get_hash(file_path.to_str().unwrap(), helper::HashType::Sha512).unwrap();
    let second_run =
        helper::get_hash(file_path.to_str().unwrap(), helper::HashType::Sha512).unwrap();
    assert_eq!(calculated, second_run);
    assert_eq!(calculated.len(), 128);
}

#[test]
fn test_directory_hashing() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();

    fs::write(dir.path().join("a.txt"), "content-a").unwrap();
    fs::write(sub.join("b.txt"), "content-b").unwrap();

    let hash1 = hash::calculate_dir_hash(dir.path()).unwrap();
    let hash2 = hash::calculate_dir_hash(dir.path()).unwrap();

    assert_eq!(hash1, hash2, "Directory hashing must be deterministic");

    fs::write(sub.join("b.txt"), "content-b-changed").unwrap();
    let hash3 = hash::calculate_dir_hash(dir.path()).unwrap();
    assert_ne!(hash1, hash3, "Hash must change when content changes");
}

#[test]
fn test_builtin_pgp_loading() {
    let res = pgp::ensure_builtin_keys();
    assert!(res.is_ok());
}
