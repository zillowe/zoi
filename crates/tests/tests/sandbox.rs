#![cfg(target_os = "linux")]
use anyhow::Result;
use std::path::Path;
use zoi::pkg::types::SandboxConfig;

fn bwrap_exists() -> bool {
    std::process::Command::new("bwrap")
        .arg("--version")
        .output()
        .is_ok()
}

#[test]
fn test_sandbox_disabled() -> Result<()> {
    let config = SandboxConfig {
        enabled: false,
        ..Default::default()
    };

    let cmd = zoi::pkg::sandbox::wrap_command(
        Path::new("echo"),
        &["hello".to_string()],
        &config,
        Path::new("/tmp"),
    )?;

    assert_eq!(cmd.get_program(), "echo");
    Ok(())
}

#[test]
fn test_sandbox_network_isolation() -> Result<()> {
    if !bwrap_exists() {
        println!("Skipping test: bwrap not installed");
        return Ok(());
    }

    let config = SandboxConfig {
        enabled: true,
        network: false,
        system: true,
        ..Default::default()
    };

    let mut cmd = zoi::pkg::sandbox::wrap_command(
        Path::new("/bin/ping"),
        &["-c".to_string(), "1".to_string(), "1.1.1.1".to_string()],
        &config,
        Path::new("/tmp"),
    )?;

    let status = cmd.status()?;
    assert!(!status.success());

    Ok(())
}

#[test]
fn test_sandbox_system_isolation() -> Result<()> {
    if !bwrap_exists() {
        println!("Skipping test: bwrap not installed");
        return Ok(());
    }

    let config = SandboxConfig {
        enabled: true,
        system: false,
        ..Default::default()
    };

    let mut cmd = zoi::pkg::sandbox::wrap_command(
        Path::new("/usr/bin/env"),
        &[],
        &config,
        Path::new("/tmp"),
    )?;

    let status = cmd.status();
    assert!(status.is_err() || !status.unwrap().success());

    Ok(())
}

#[test]
fn test_sandbox_tmpfs_isolation() -> Result<()> {
    if !bwrap_exists() {
        println!("Skipping test: bwrap not installed");
        return Ok(());
    }

    let config = SandboxConfig {
        enabled: true,
        system: true,
        ..Default::default()
    };

    let host_tmp_file = "/tmp/zoi_sandbox_test_file";
    std::fs::write(host_tmp_file, "secret")?;

    let mut cmd = zoi::pkg::sandbox::wrap_command(
        Path::new("/bin/cat"),
        &[host_tmp_file.to_string()],
        &config,
        Path::new("/opt"),
    )?;

    let output = cmd.output()?;

    let _ = std::fs::remove_file(host_tmp_file);

    assert!(!output.status.success());

    Ok(())
}

#[test]
fn test_sandbox_env_passthrough() -> Result<()> {
    if !bwrap_exists() {
        println!("Skipping test: bwrap not installed");
        return Ok(());
    }

    unsafe {
        std::env::set_var("ZOI_TEST_SECRET_ENV", "my_secret_value");
    }

    let config = SandboxConfig {
        enabled: true,
        system: true,
        env: vec!["ZOI_TEST_SECRET_ENV".to_string()],
        ..Default::default()
    };

    let mut cmd = zoi::pkg::sandbox::wrap_command(
        Path::new("/usr/bin/env"),
        &[],
        &config,
        Path::new("/opt"),
    )?;

    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("ZOI_TEST_SECRET_ENV=my_secret_value"));

    unsafe {
        std::env::remove_var("ZOI_TEST_SECRET_ENV");
    }

    Ok(())
}
