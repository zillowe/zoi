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

use std::path::PathBuf;

pub fn setup_path() -> Result<(), Box<dyn Error>> {
    let zoi_bin_dir = home::home_dir()
        .ok_or("Could not find home directory.")?
        .join(".zoi")
        .join("pkgs")
        .join("bin");

    if !zoi_bin_dir.exists() {
        fs::create_dir_all(&zoi_bin_dir)?;
    }

    println!("{}", "Ensuring Zoi bin directory is in your PATH...".bold());

    #[cfg(unix)]
    {
        use std::fs::{File, OpenOptions};
        let zoi_bin_str = "$HOME/.zoi/pkgs/bin";

        let shell_name = std::env::var("SHELL").unwrap_or_default();
        let profile_file_path = if shell_name.contains("bash") {
            if cfg!(target_os = "macos") {
                home::home_dir().unwrap().join(".bash_profile")
            } else {
                home::home_dir().unwrap().join(".bashrc")
            }
        } else if shell_name.contains("zsh") {
            home::home_dir().unwrap().join(".zshrc")
        } else if shell_name.contains("fish") {
            home::home_dir().unwrap().join(".config/fish/config.fish")
        } else {
            home::home_dir().unwrap().join(".profile")
        };

        if !profile_file_path.exists() {
            File::create(&profile_file_path)?;
        }

        let content = fs::read_to_string(&profile_file_path)?;
        if content.contains(zoi_bin_str) {
            println!("Zoi bin directory is already in your shell's config.");
            return Ok(());
        }

        let mut file = OpenOptions::new().append(true).open(&profile_file_path)?;

        let cmd_to_write = if shell_name.contains("fish") {
            format!("\n# Added by Zoi\nset -gx PATH \"{}\" $PATH\n", zoi_bin_str)
        } else {
            format!("\n# Added by Zoi\nexport PATH=\"{}:$PATH\"\n", zoi_bin_str)
        };

        file.write_all(cmd_to_write.as_bytes())?;

        println!(
            "{} Zoi bin directory has been added to your PATH in '{}'.",
            "Success:".green(),
            profile_file_path.display()
        );
        println!(
            "Please restart your shell or run `source {}` for the changes to take effect.",
            profile_file_path.display()
        );
    }

    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let zoi_bin_path_str = zoi_bin_dir.to_str().ok_or("Invalid path string")?;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;
        let current_path: String = env.get_value("Path")?;

        if current_path
            .split(';')
            .any(|p| p.eq_ignore_ascii_case(zoi_bin_path_str))
        {
            println!("Zoi bin directory is already in your PATH.");
            return Ok(());
        }

        let new_path = if current_path.is_empty() {
            zoi_bin_path_str.to_string()
        } else {
            format!("{};{}", current_path, zoi_bin_path_str)
        };

        env.set_value("Path", &new_path)?;

        println!(
            "{} Zoi bin directory has been added to your user PATH environment variable.",
            "Success:".green()
        );
        println!(
            "Please restart your shell or log out and log back in for the changes to take effect."
        );
    }

    Ok(())
}

pub fn check_path() {
    if let Some(home) = home::home_dir() {
        let zoi_bin_dir = home.join(".zoi/pkgs/bin");
        if !zoi_bin_dir.exists() {
            return;
        }

        if let Ok(path_var) = std::env::var("PATH") {
            let zoi_bin_dir_canon =
                fs::canonicalize(&zoi_bin_dir).unwrap_or_else(|_| zoi_bin_dir.clone());

            let is_in_path = path_var.split(std::path::MAIN_SEPARATOR_STR).any(|p| {
                if p.is_empty() {
                    return false;
                }
                let p_path = PathBuf::from(p);

                if p_path == zoi_bin_dir {
                    return true;
                }

                if let Ok(p_canon) = fs::canonicalize(&p_path) {
                    if p_canon == zoi_bin_dir_canon {
                        return true;
                    }
                }

                false
            });

            if !is_in_path {
                eprintln!(
                    "{}: zoi's bin directory `{}` is not in your PATH.",
                    "Warning".yellow(),
                    zoi_bin_dir.display()
                );
                eprintln!("Please restart your terminal, or add it to your PATH manually for commands to be available.");
            }
        }
    }
}

pub fn get_platform() -> Result<String, String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        unsupported_os => return Err(format!("Unsupported operating system: {}", unsupported_os)),
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        unsupported_arch => return Err(format!("Unsupported architecture: {}", unsupported_arch)),
    };

    Ok(format!("{}-{}", os, arch))
}
