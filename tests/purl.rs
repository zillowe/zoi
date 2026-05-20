use std::process::Command;

#[test]
fn test_cli_install_purl_flag() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "install",
            "--purl",
            "pkg:zoi/zoidberg/main/athas@0.6.0",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("Proceed with installation? [y/N]") || stdout.contains("athas"),
        "Expected output to show installation details or prompt. stdout: {}\nstderr: {}",
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
            "pkg:zoi/zoidberg/zillowe/hello@4.0.0",
        ])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Command failed: stdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("athas 0.6.0"),
        "Expected package name and version in output: {}",
        stdout
    );
}

#[test]
fn test_cli_purl_missing_repo() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "show",
            "--purl",
            "pkg:zoi/zoidberg/hello@4.0.0",
        ])
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected command to fail");
    assert!(
        stderr.contains("PURL missing repository path"),
        "Expected error message for missing repo. stderr: {}",
        stderr
    );
}

#[test]
fn test_cli_purl_repo_mismatch() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "show",
            "--purl",
            "pkg:zoi/zoidberg/asadasd/hello@4.0.0",
        ])
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "Expected command to fail");
    assert!(
        stderr.contains("Repository mismatch in PURL"),
        "Expected error message for repo mismatch. stderr: {}",
        stderr
    );
}
