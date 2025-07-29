use crate::pkg::resolve::SourceType;
use colored::*;
use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::process::Command;

pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        use std::mem;
        use std::ptr;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::processthreadsapi::OpenProcessToken;
        use winapi::um::securitybaseapi::CheckTokenMembership;
        use winapi::um::securitybaseapi::CreateWellKnownSid;
        use winapi::um::winnt::{PSID, TOKEN_QUERY};

        let mut token = ptr::null_mut();
        let process = unsafe { GetCurrentProcess() };
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return false;
        }

        let mut sid: [u8; 8] = [0; 8];
        let mut sid_size = mem::size_of_val(&sid) as u32;
        if unsafe {
            winapi::um::securitybaseapi::CreateWellKnownSid(
                winapi::um::winnt::WinBuiltinAdministratorsSid,
                ptr::null_mut(),
                sid.as_mut_ptr() as PSID,
                &mut sid_size,
            )
        } == 0
        {
            unsafe { CloseHandle(token) };
            return false;
        }

        let mut is_member = 0;
        let result =
            unsafe { CheckTokenMembership(token, sid.as_mut_ptr() as PSID, &mut is_member) };
        unsafe { CloseHandle(token) };

        result != 0 && is_member != 0
    }
    #[cfg(unix)]
    {
        nix::unistd::getuid().is_root()
    }
}

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
                    "arch" | "manjaro" | "cachyos" => Some("pacman".to_string()),
                    "ubuntu" | "debian" | "linuxmint" | "pop" | "kali" => Some("apt".to_string()),
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
        "freebsd" => Some("pkg".to_string()),
        "openbsd" => Some("pkg_add".to_string()),
        _ => None,
    }
}

pub fn print_repo_warning(repo_name: &Option<String>) {
    if let Some(repo) = repo_name {
        let warning_message = match repo.as_str() {
            "test" => Some("This package is in a testing phase and may not function correctly."),
            "archive" => Some("This package is outdated and no longer receives updates."),
            _ => None,
        };

        if let Some(message) = warning_message {
            println!("\n{}: {}", "NOTE".yellow().bold(), message.yellow());
        }
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

use crate::pkg::types::Scope;
use std::path::PathBuf;

pub fn setup_path(scope: Scope) -> Result<(), Box<dyn Error>> {
    let zoi_bin_dir = match scope {
        Scope::User => home::home_dir()
            .ok_or("Could not find home directory.")?
            .join(".zoi")
            .join("pkgs")
            .join("bin"),
        Scope::System => {
            if cfg!(target_os = "windows") {
                PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin")
            } else {
                PathBuf::from("/usr/local/bin")
            }
        }
    };

    if !zoi_bin_dir.exists() {
        fs::create_dir_all(&zoi_bin_dir)?;
    }

    if scope == Scope::System {
        println!(
            "{}",
            "System-wide installation complete. Binaries are in the system PATH.".green()
        );
        return Ok(());
    }

    println!("{}", "Ensuring Zoi bin directory is in your PATH...".bold());

    #[cfg(unix)]
    {
        use std::fs::{File, OpenOptions};
        let home = home::home_dir().ok_or("Could not find home directory.")?;
        let zoi_bin_str = "$HOME/.zoi/pkgs/bin";

        let shell_name = std::env::var("SHELL").unwrap_or_default();
        let (profile_file_path, cmd_to_write) = if shell_name.contains("bash") {
            let path = if cfg!(target_os = "macos") {
                home.join(".bash_profile")
            } else {
                home.join(".bashrc")
            };
            let cmd = format!("\n# Added by Zoi\nexport PATH=\"{}:$PATH\"\n", zoi_bin_str);
            (path, cmd)
        } else if shell_name.contains("zsh") {
            let path = home.join(".zshrc");
            let cmd = format!("\n# Added by Zoi\nexport PATH=\"{}:$PATH\"\n", zoi_bin_str);
            (path, cmd)
        } else if shell_name.contains("fish") {
            let path = home.join(".config/fish/config.fish");
            let cmd = format!("\n# Added by Zoi\nset -gx PATH \"{}\" $PATH\n", zoi_bin_str);
            (path, cmd)
        } else if shell_name.contains("csh") || shell_name.contains("tcsh") {
            let path = home.join(".cshrc");
            let cmd = format!("\n# Added by Zoi\nsetenv PATH \"{}:$PATH\"\n", zoi_bin_str);
            (path, cmd)
        } else {
            let path = home.join(".profile");
            let cmd = format!("\n# Added by Zoi\nexport PATH=\"{}:$PATH\"\n", zoi_bin_str);
            (path, cmd)
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
        use winreg::RegKey;
        use winreg::enums::*;

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

            let _is_in_path = path_var.split(std::path::MAIN_SEPARATOR).any(|p| {
                if p.is_empty() {
                    return false;
                }

                let p_expanded = if p.starts_with("~/") {
                    home.join(&p[2..])
                } else if p == "~" {
                    home.clone()
                } else {
                    PathBuf::from(p)
                };

                if let Ok(p_canon) = fs::canonicalize(&p_expanded) {
                    if p_canon == zoi_bin_dir_canon {
                        return true;
                    }
                }

                false
            });

            // if !is_in_path {
            //     eprintln!(
            //         "{}: zoi's bin directory `{}` is not in your PATH.",
            //         "Warning".yellow(),
            //         zoi_bin_dir.display()
            //     );
            //     eprintln!(
            //         "Please restart your terminal, or add it to your PATH manually for commands to be available."
            //     );
            // }
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
