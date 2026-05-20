use std::io::Write;
use std::process::Command;

#[test]
fn test_helper_get_hash_sha256() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "hello world").expect("Failed to write");
    let file_path = temp_file.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args([
            "run", "--", "helper", "get-hash", file_path, "--hash", "sha256",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"));
}

#[test]
fn test_helper_get_hash_sha512() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "hello world").expect("Failed to write");
    let file_path = temp_file.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args([
            "run", "--", "helper", "get-hash", file_path, "--hash", "sha512",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"));
}

#[test]
fn test_helper_validate_registries_json() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "helper",
            "validate",
            "tests/assets/registries.json",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("file is a valid registries.json spec"));
}

#[test]
fn test_helper_validate_packages_json() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "helper",
            "validate",
            "tests/assets/packages.json",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("file is a valid packages.json spec"));
}

#[test]
fn test_helper_validate_repo_yaml() {
    let output = Command::new("cargo")
        .args(["run", "--", "helper", "validate", "tests/assets/repo.yaml"])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("file is a valid repo.yaml spec"));
}

#[test]
fn test_helper_validate_advisories_json() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "helper",
            "validate",
            "tests/assets/advisories.json",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("file is a valid advisories.json spec"));
}

#[test]
fn test_helper_validate_sec_yaml() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "helper",
            "validate",
            "tests/assets/ZSA-2026-D0042.sec.yaml",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("file is a valid .sec.yaml spec"));
}

#[test]
fn test_helper_validate_invalid_file() {
    let mut temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(temp_file, "invalid content").expect("Failed to write");
    let file_path = temp_file.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args(["run", "--", "helper", "validate", file_path])
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("Unsupported file extension. Please provide a .json or .yaml file"));
}
