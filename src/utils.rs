use crate::pkg::resolve::SourceType;
use colored::*;
use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::process::Command;

pub fn print_info<T: Display>(key: &str, value: T) {
    println!("{}: {}", key.cyan(), value);
}

pub fn format_version_summary(branch: &str, status: &str, number: &str) -> String {
    let branch_short = if branch == "Production" {
        "Prod."
    } else if branch == "Development" {
        "Dev."
    } else {
        branch
    };
    format!(
        "{} {} {}",
        branch_short.blue().bold().italic(),
        status,
        number,
    )
}

pub fn format_version_full(branch: &str, status: &str, number: &str, commit: &str) -> String {
    format!(
        "{} {}",
        format_version_summary(branch, status, number),
        commit.green()
    )
}

pub fn print_aligned_info(key: &str, value: &str) {
    let key_with_colon = format!("{key}:");
    println!("{:<18}{}", key_with_colon.cyan(), value);
}

pub fn command_exists(command: &str) -> bool {
    if cfg!(target_os = "windows") {
        Command::new("where")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {command}"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

pub fn ask_for_confirmation(prompt: &str, yes: bool) -> bool {
    if yes {
        return true;
    }
    print!("{} [y/N]: ", prompt.yellow());
    let _ = stdout().flush();
    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}

pub fn get_linux_distribution() -> Option<String> {
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        for line in contents.lines() {
            if let Some(id) = line.strip_prefix("ID=") {
                return Some(id.trim_matches('"').to_string());
            }
        }
    }
    None
}

pub fn get_native_package_manager() -> Option<String> {
    let os = std::env::consts::OS;
    match os {
        "linux" => {
            if let Some(distro) = get_linux_distribution() {
                match distro.as_str() {
                    "arch" => Some("pacman".to_string()),
                    "ubuntu" | "debian" | "linuxmint" | "pop" => Some("apt".to_string()),
                    "fedora" | "centos" | "rhel" => Some("dnf".to_string()),
                    "opensuse-tumbleweed" | "opensuse-leap" => Some("zypper".to_string()),
                    "alpine" => Some("apk".to_string()),
                    _ => None,
                }
            } else {
                None
            }
        }
        "macos" => {
            if command_exists("brew") {
                Some("brew".to_string())
            } else {
                None
            }
        }
        "windows" => {
            if command_exists("scoop") {
                Some("scoop".to_string())
            } else if command_exists("choco") {
                Some("choco".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn confirm_untrusted_source(source_type: &SourceType, yes: bool) -> Result<(), Box<dyn Error>> {
    if source_type == &SourceType::OfficialRepo {
        return Ok(());
    }

    let warning_message = match source_type {
        SourceType::UntrustedRepo(repo) => {
            format!("The package from repository '@{repo}' is not an official Zoi repository.")
        }
        SourceType::LocalFile => "You are installing from a local file.".to_string(),
        SourceType::Url => "You are installing from a remote URL.".to_string(),
        _ => return Ok(()),
    };

    println!(
        "\n{}: {}",
        "SECURITY WARNING".yellow().bold(),
        warning_message
    );

    if ask_for_confirmation(
        "This source is not trusted. Are you sure you want to continue?",
        yes,
    ) {
        Ok(())
    } else {
        Err("Operation aborted by user.".into())
    }
}

pub fn is_platform_compatible(current_platform: &str, allowed_platforms: &[String]) -> bool {
    let os = match std::env::consts::OS {
        "darwin" => "macos",
        other => other,
    };
    allowed_platforms
        .iter()
        .any(|p| p == "all" || p == os || p == current_platform)
}

pub fn get_platform() -> Result<String, String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        unsupported_os => return Err(format!("Unsupported operating system: {}", unsupported_os)),
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        unsupported_arch => return Err(format!("Unsupported architecture: {}", unsupported_arch)),
    };

    Ok(format!("{}-{}", os, arch))
}
