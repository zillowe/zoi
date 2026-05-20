use std::process::Command;

#[test]
fn test_cli_install_purl_flag() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "install",
            "--purl",
            "pkg:zoi/mock_registry/mock_repo/mock_pkg@1.0.0",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Fetching PURL package")
            || stderr.contains("Failed to fetch central Zoi registry database"),
        "Expected output indicating PURL fetch attempt. stdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_cli_show_purl_flag() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "show",
            "--purl",
            "pkg:zoi/mock_registry/mock_repo/mock_pkg@1.0.0",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Fetching PURL package")
            || stderr.contains("Failed to fetch central Zoi registry database"),
        "Expected output indicating PURL fetch attempt. stdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}
