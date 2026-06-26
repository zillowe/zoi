use std::io::Write;
use std::path::PathBuf;
use zoi::pkg::helper;

#[test]
fn test_helper_get_hash_sha256() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "hello world").expect("Failed to write");
    let file_path = temp_file.path().to_str().unwrap();

    let hash = helper::get_hash(file_path, helper::HashType::Sha256).expect("get_hash failed");
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

#[test]
fn test_helper_get_hash_sha512() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "hello world").expect("Failed to write");
    let file_path = temp_file.path().to_str().unwrap();

    let hash = helper::get_hash(file_path, helper::HashType::Sha512).expect("get_hash failed");
    assert_eq!(
        hash,
        "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
    );
}

#[test]
fn test_helper_validate_registries_json() {
    let res = helper::validate::run(&PathBuf::from("tests/assets/registries.json"));
    assert!(res.is_ok(), "Validation failed: {:?}", res);
}

#[test]
fn test_helper_validate_packages_json() {
    let res = helper::validate::run(&PathBuf::from("tests/assets/packages.json"));
    assert!(res.is_ok(), "Validation failed: {:?}", res);
}

#[test]
fn test_helper_validate_repo_yaml() {
    let res = helper::validate::run(&PathBuf::from("tests/assets/repo.yaml"));
    assert!(res.is_ok(), "Validation failed: {:?}", res);
}

#[test]
fn test_helper_validate_advisories_json() {
    let res = helper::validate::run(&PathBuf::from("tests/assets/advisories.json"));
    assert!(res.is_ok(), "Validation failed: {:?}", res);
}

#[test]
fn test_helper_validate_sec_yaml() {
    let res = helper::validate::run(&PathBuf::from("tests/assets/ZSA-2026-D0042.sec.yaml"));
    assert!(res.is_ok(), "Validation failed: {:?}", res);
}

#[test]
fn test_helper_validate_invalid_file() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "invalid content").expect("Failed to write");

    let res = helper::validate::run(temp_file.path());
    assert!(res.is_err(), "Expected validation to fail");
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("Unsupported file extension")
    );
}
