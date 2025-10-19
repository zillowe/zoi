use crate::pkg::resolve::SourceType;
use anyhow::anyhow;
use colored::*;
use std::fmt::Display;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::process::Command;
use std::time::Duration;
use walkdir::WalkDir;

use crate::pkg::types::Scope;
use clap_complete::Shell;
use std::path::{Path, PathBuf};

pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        use std::mem;
        use std::ptr;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::processthreadsapi::OpenProcessToken;
        use winapi::um::securitybaseapi::CheckTokenMembership;
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
    println!("{}: {}", key, value);
}

pub fn format_version_summary(branch: &str, status: &str, number: &str) -> String {
    let branch_short = if branch == "Production" {
        "Prod."
    } else if branch == "Development" {
        "Dev."
    } else if branch == "Public" {
        "Pub."
    } else if branch == "Special" {
        "Spec."
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
    let key_with_colon = format!("{}:", key);
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
        Command::new("bash")
            .arg("-c")
            .arg(format!("command -v {}", command))
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

use std::collections::HashMap;

pub fn get_linux_distribution_info() -> Option<HashMap<String, String>> {
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        let info: HashMap<String, String> = contents
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, '=');
                let key = parts.next()?;
                let value = parts.next()?.trim_matches('"').to_string();
                if key.is_empty() {
                    None
                } else {
                    Some((key.to_string(), value))
                }
            })
            .collect();
        if info.is_empty() { None } else { Some(info) }
    } else {
        None
    }
}

pub fn get_linux_distro_family() -> Option<String> {
    if let Some(info) = get_linux_distribution_info() {
        if let Some(id_like) = info.get("ID_LIKE") {
            let families: Vec<&str> = id_like.split_whitespace().collect();
            if families.contains(&"debian") {
                return Some("debian".to_string());
            }
            if families.contains(&"arch") {
                return Some("arch".to_string());
            }
            if families.contains(&"fedora") {
                return Some("fedora".to_string());
            }
            if families.contains(&"rhel") {
                return Some("fedora".to_string());
            }
            if families.contains(&"suse") {
                return Some("suse".to_string());
            }
            if families.contains(&"gentoo") {
                return Some("gentoo".to_string());
            }
        }
        if let Some(id) = info.get("ID") {
            return match id.as_str() {
                "debian" | "ubuntu" | "linuxmint" | "pop" | "kali" | "kubuntu" | "lubuntu"
                | "xubuntu" | "zorin" | "elementary" => Some("debian".to_string()),
                "arch" | "manjaro" | "cachyos" | "endeavouros" | "garuda" => {
                    Some("arch".to_string())
                }
                "fedora" | "centos" | "rhel" | "rocky" | "almalinux" => Some("fedora".to_string()),
                "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" => Some("suse".to_string()),
                "gentoo" => Some("gentoo".to_string()),
                "alpine" => Some("alpine".to_string()),
                "void" => Some("void".to_string()),
                "solus" => Some("solus".to_string()),
                "guix" => Some("guix".to_string()),
                _ => None,
            };
        }
    }
    None
}

pub fn get_linux_distribution() -> Option<String> {
    get_linux_distribution_info().and_then(|info| info.get("ID").cloned())
}

pub fn get_native_package_manager() -> Option<String> {
    let os = std::env::consts::OS;
    match os {
        "linux" => get_linux_distro_family()
            .map(|family| {
                match family.as_str() {
                    "debian" => "apt",
                    "arch" => "pacman",
                    "fedora" => "dnf",
                    "suse" => "zypper",
                    "gentoo" => "portage",
                    "alpine" => "apk",
                    "void" => "xbps-install",
                    "solus" => "eopkg",
                    "guix" => "guix",
                    _ => "unknown",
                }
                .to_string()
            })
            .filter(|s| s != "unknown"),
        "macos" => {
            if command_exists("brew") {
                Some("brew".to_string())
            } else if command_exists("port") {
                Some("macports".to_string())
            } else {
                None
            }
        }
        "windows" => {
            if command_exists("scoop") {
                Some("scoop".to_string())
            } else if command_exists("choco") {
                Some("choco".to_string())
            } else if command_exists("winget") {
                Some("winget".to_string())
            } else {
                None
            }
        }
        "freebsd" => Some("pkg".to_string()),
        "openbsd" => Some("pkg_add".to_string()),
        _ => None,
    }
}

pub fn print_repo_warning(repo_name: &str) {
    if let Ok(db_path) = crate::pkg::resolve::get_db_root()
        && let Ok(repo_config) = crate::pkg::config::read_repo_config(&db_path)
    {
        let major_repo = repo_name.split('/').next().unwrap_or("");
        if let Some(repo_entry) = repo_config.repos.iter().find(|r| r.name == major_repo) {
            let warning_message = match repo_entry.repo_type.as_str() {
                "unoffical" => {
                    Some("This package is from an unofficial repository and is not trusted.")
                }
                "community" => {
                    Some("This package is from a community repository. Use with caution.")
                }
                "test" => Some(
                    "This package is from a testing repository and may not function correctly.",
                ),
                "archive" => {
                    Some("This package is from an archive repository and is no longer maintained.")
                }
                _ => None,
            };

            if let Some(message) = warning_message {
                println!("\n{}: {}", "NOTE".yellow().bold(), message.yellow());
            }
        }
    }
}

pub fn confirm_untrusted_source(source_type: &SourceType, yes: bool) -> anyhow::Result<()> {
    if source_type == &SourceType::OfficialRepo {
        return Ok(());
    }

    let warning_message = match source_type {
        SourceType::UntrustedRepo(repo) => {
            format!(
                "The package from repository '@{}' is not an official Zoi repository.",
                repo
            )
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
        Err(anyhow!("Operation aborted by user."))
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

pub fn setup_path(scope: Scope) -> anyhow::Result<()> {
    if scope == Scope::Project {
        return Ok(());
    }

    let zoi_bin_dir = match scope {
        Scope::User => home::home_dir()
            .ok_or_else(|| anyhow!("Could not find home directory."))?
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
        Scope::Project => return Ok(()),
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
        let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
        let zoi_bin_str = "$HOME/.zoi/pkgs/bin";

        let shell_name = std::env::var("SHELL").unwrap_or_default();
        let (profile_file_path, cmd_to_write) = if shell_name.contains("bash") {
            let path = if cfg!(target_os = "macos") {
                home.join(".bash_profile")
            } else {
                home.join(".bashrc")
            };
            let cmd = format!(
                "\n# Added by Zoi\nexport PATH=\"{}:{}\"\n",
                zoi_bin_str, "$PATH"
            );
            (path, cmd)
        } else if shell_name.contains("zsh") {
            let path = home.join(".zshrc");
            let cmd = format!(
                "\n# Added by Zoi\nexport PATH=\"{}:{}\"\n",
                zoi_bin_str, "$PATH"
            );
            (path, cmd)
        } else if shell_name.contains("fish") {
            let path = home.join(".config/fish/config.fish");
            let cmd = format!("\n# Added by Zoi\nset -gx PATH \"{}\" $PATH\n", zoi_bin_str);

            (path, cmd)
        } else if shell_name.contains("elvish") {
            let path = home.join(".config/elvish/rc.elv");
            let cmd = "
# Added by Zoi
set paths = [ ~/.zoi/pkgs/bin $paths... ]
"
            .to_string();
            (path, cmd)
        } else if shell_name.contains("csh") || shell_name.contains("tcsh") {
            let path = home.join(".cshrc");
            let cmd = format!(
                "\n# Added by Zoi\nsetenv PATH=\"{}:{}\"\n",
                zoi_bin_str, "$PATH"
            );
            (path, cmd)
        } else {
            let path = home.join(".profile");
            let cmd = format!(
                "\n# Added by Zoi\nexport PATH=\"{}:{}\"\n",
                zoi_bin_str, "$PATH"
            );
            (path, cmd)
        };

        if !profile_file_path.exists() {
            if let Some(parent) = profile_file_path.parent() {
                fs::create_dir_all(parent)?;
            }
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

        let zoi_bin_path_str = zoi_bin_dir
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path string"))?;

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
    } else {
        return;
    }

    let command_output = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("-Command")
            .arg("echo $env:Path")
            .output()
    } else {
        Command::new("bash").arg("-c").arg("echo $PATH").output()
    };

    let is_in_path = match command_output {
        Ok(output) => {
            if output.status.success() {
                let path_var = String::from_utf8_lossy(&output.stdout);
                path_var.contains(".zoi/pkgs/bin")
            } else {
                false
            }
        }
        Err(_) => false,
    };

    if !is_in_path {
        eprintln!(
            "Please run 'zoi setup --scope user' or add it to your PATH manually for commands to be available."
        );
    }
}

pub fn get_platform() -> anyhow::Result<String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        unsupported_os => return Err(anyhow!("Unsupported operating system: {}", unsupported_os)),
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        unsupported_arch => return Err(anyhow!("Unsupported architecture: {}", unsupported_arch)),
    };

    Ok(format!("{}-{}", os, arch))
}

pub fn get_all_available_package_managers() -> Vec<String> {
    let mut managers = Vec::new();
    let all_possible_managers = [
        "apt",
        "apt-get",
        "pacman",
        "yay",
        "paru",
        "pikaur",
        "trizen",
        "dnf",
        "yum",
        "zypper",
        "portage",
        "apk",
        "snap",
        "flatpak",
        "nix",
        "brew",
        "port",
        "scoop",
        "choco",
        "winget",
        "pkg",
        "pkg_add",
        "xbps-install",
        "eopkg",
        "guix",
        "mas",
    ];

    for manager in &all_possible_managers {
        if command_exists(manager) {
            managers.push(manager.to_string());
        }
    }
    managers.sort();
    managers.dedup();
    managers
}

pub fn build_blocking_http_client(timeout_secs: u64) -> anyhow::Result<reqwest::blocking::Client> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?;
    Ok(client)
}

pub fn retry_backoff_sleep(attempt: u32) {
    let base_ms = 500u64.saturating_mul(1u64 << (attempt.saturating_sub(1)));
    let jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .subsec_millis()
        % 200) as u64;
    let sleep_ms = (base_ms + jitter).min(8000);
    std::thread::sleep(Duration::from_millis(sleep_ms));
}

pub fn check_license(license: &str) {
    if license.is_empty() {
        println!(
            "{}",
            "Warning: Package does not have a license specified.".yellow()
        );
        return;
    }

    if license.eq_ignore_ascii_case("Proprietary") {
        println!(
            "{}",
            "Warning: Package is using a proprietary license.".red()
        );
        return;
    }

    match spdx::Expression::parse(license) {
        Ok(expr) => {
            if !expr.evaluate(|req| match req.license {
                spdx::LicenseItem::Spdx { id, .. } => id.is_osi_approved(),
                spdx::LicenseItem::Other { .. } => false,
            }) {
                println!(
                    "{}{}{}",
                    "Warning: License '".yellow(),
                    license.yellow().bold(),
                    "' is not an OSI approved license.".yellow()
                );
            }
        }
        Err(_) => {
            println!(
                "{}{}{}",
                "Warning: Could not parse license expression '".yellow(),
                license.yellow().bold(),
                "' It may not be a valid SPDX identifier.".yellow()
            );
        }
    }
}

#[derive(serde::Deserialize)]
struct PackageForCompletion {
    description: Option<String>,
}

pub struct PackageCompletion {
    pub display: String,
    pub repo: String,
    pub description: String,
}

pub fn get_all_packages_for_completion() -> Vec<PackageCompletion> {
    let db_root = if let Ok(path) = crate::pkg::resolve::get_db_root() {
        path
    } else {
        return Vec::new();
    };

    let active_repos = if let Ok(config) = crate::pkg::config::read_config() {
        config.repos
    } else {
        return Vec::new();
    };

    if !db_root.exists() {
        return Vec::new();
    }

    let mut packages = Vec::new();
    for repo_name in &active_repos {
        let repo_path = db_root.join(repo_name);
        if !repo_path.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            let pkg_name = entry.file_name().to_string_lossy();
            let pkg_file_path = entry.path().join(format!("{}.pkg.lua", pkg_name));

            if pkg_file_path.is_file() {
                let pkg_info: anyhow::Result<PackageForCompletion> = (|| -> anyhow::Result<_> {
                    let pkg = crate::pkg::lua::parser::parse_lua_package(
                        pkg_file_path.to_str().unwrap(),
                        None,
                    )?;
                    Ok(PackageForCompletion {
                        description: Some(pkg.description),
                    })
                })();

                let description = match pkg_info {
                    Ok(pi) => pi.description.unwrap_or_default(),
                    Err(_) => String::new(),
                };

                let relative_path = entry.path().strip_prefix(&db_root).unwrap();
                let full_pkg_id =
                    format!("@{}", relative_path.to_string_lossy().replace('\\', "/"));

                packages.push(PackageCompletion {
                    display: full_pkg_id,
                    repo: repo_name.clone(),
                    description,
                });
            }
        }
    }
    packages.sort_by(|a, b| a.display.cmp(&b.display));
    packages
}

pub fn get_current_shell() -> Option<Shell> {
    if cfg!(windows) {
        return Some(Shell::PowerShell);
    }

    if let Ok(shell_path) = std::env::var("SHELL") {
        let shell_name = Path::new(&shell_path).file_name()?.to_str()?;
        match shell_name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "elvish" => Some(Shell::Elvish),
            "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    } else {
        None
    }
}
