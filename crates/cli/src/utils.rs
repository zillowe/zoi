use crate::pkg::types::Scope;
use anyhow::anyhow;
use colored::*;
use crossterm::tty::IsTty;
use std::fmt::Display;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn print_repo_warning(repo_name: &str) {
    if crate::pkg::utils::is_mini_mode() {
        if let Ok(index) = crate::pkg::mini_resolve::fetch_registry_index()
            && let Some(pkg_info) = index.packages.values().find(|p| p.repo == repo_name)
        {
            let warning_message = match pkg_info.repo_type.as_str() {
                "unofficial" => {
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
        return;
    }

    if let Ok(db_path) = crate::pkg::resolve::get_db_root()
        && let Ok(repo_config) = crate::pkg::config::read_repo_config(&db_path)
    {
        let major_repo = repo_name.split('/').next().unwrap_or_default();
        if let Some(repo_entry) = repo_config.repos.iter().find(|r| r.name == major_repo) {
            let warning_message = match repo_entry.repo_type.as_str() {
                "unofficial" => {
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

pub fn get_all_packages_for_completion() -> Vec<PackageCompletion> {
    let mut completions = Vec::new();
    let config = if let Ok(cfg) = crate::pkg::config::read_config() {
        cfg
    } else {
        return completions;
    };

    let mut registries = Vec::new();
    if let Some(default) = &config.default_registry {
        registries.push(default.handle.clone());
    }
    for reg in &config.added_registries {
        registries.push(reg.handle.clone());
    }

    let default_handle = config.default_registry.as_ref().map(|r| &r.handle);

    for handle in registries {
        if handle.is_empty() {
            continue;
        }
        if let Ok(entries) = crate::pkg::db::get_packages_for_completion(&handle) {
            let is_default = default_handle == Some(&handle);
            for entry in entries {
                let base_name = if is_default {
                    format!("@{}/{}", entry.repo, entry.name)
                } else {
                    format!("#{}@{}/{}", handle, entry.repo, entry.name)
                };

                let display = if let Some(sub) = entry.sub_package {
                    format!("{}:{}", base_name, sub)
                } else {
                    base_name
                };

                completions.push(PackageCompletion {
                    display,
                    repo: entry.repo,
                    description: entry.description,
                });
            }
        }
    }

    completions.sort_by(|a, b| a.display.cmp(&b.display));
    completions
}

pub struct PackageCompletion {
    pub display: String,
    pub repo: String,
    pub description: String,
}

pub fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    if link.exists() || link.is_symlink() {
        fs::remove_file(link)?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(target, link).is_err() {
            if fs::hard_link(target, link).is_err() {
                fs::copy(target, link)?;
            }
        }
        Ok(())
    }
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

    if license.eq_ignore_ascii_case("Unknown") {
        println!("{}", "Warning: Package license is unknown.".red());
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

pub fn ask_for_confirmation(prompt: &str, yes: bool) -> bool {
    if yes {
        return true;
    }

    if std::env::var("ZOI_TEST").is_ok() || !stdin().is_tty() {
        return false;
    }

    print!("{} [y/N]: ", prompt.yellow());
    let _ = stdout().flush();
    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}

pub fn setup_path(scope: Scope) -> anyhow::Result<()> {
    if scope == Scope::Project {
        return Ok(());
    }

    let zoi_bin_dir = match scope {
        Scope::User => {
            let home = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            crate::pkg::sysroot::apply_sysroot(home.join(".zoi").join("pkgs").join("bin"))
        }
        Scope::System => {
            if cfg!(target_os = "windows") {
                crate::pkg::sysroot::apply_sysroot(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                crate::pkg::sysroot::apply_sysroot(PathBuf::from("/usr/local/bin"))
            }
        }
        Scope::Project => return Ok(()),
    };

    if !zoi_bin_dir.exists() {
        fs::create_dir_all(&zoi_bin_dir)?;
    }

    if scope == Scope::System && cfg!(unix) {
        println!(
            "{}",
            "System-wide installation complete. Binaries are in the system PATH.".green()
        );
        return Ok(());
    }

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

        let (root, subkey, scope_name) = if scope == Scope::System {
            if !is_admin() {
                return Err(anyhow!(
                    "Administrator privileges required to modify system PATH."
                ));
            }
            (
                HKEY_LOCAL_MACHINE,
                "System\\CurrentControlSet\\Control\\Session Manager\\Environment",
                "system",
            )
        } else {
            (HKEY_CURRENT_USER, "Environment", "user")
        };

        let key = RegKey::predef(root);
        let env = key.open_subkey_with_flags(subkey, KEY_READ | KEY_WRITE)?;
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
            "{} Zoi bin directory has been added to your {} PATH environment variable.",
            "Success:".green(),
            scope_name
        );
        println!(
            "Please restart your shell or log out and log back in for the changes to take effect."
        );
    }

    Ok(())
}

pub fn check_path() {
    if let Some(home) = home::home_dir() {
        let zoi_bin_dir = crate::pkg::sysroot::apply_sysroot(home.join(".zoi/pkgs/bin"));
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
            "Please run 'zoi shell <shell>' or add it to your PATH manually for commands to be available."
        );
    }
}
