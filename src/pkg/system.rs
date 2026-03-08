use crate::pkg::{extension, local, lua, service, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use openssl::rand::rand_bytes;
use openssl::symm::{Cipher, decrypt, encrypt};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn get_machine_key() -> Result<Vec<u8>> {
    let key_path = get_system_config_lua_path()
        .parent()
        .ok_or_else(|| anyhow!("Invalid system config path"))?
        .join("machine_key");

    if !key_path.exists() {
        if let Some(p) = key_path.parent() {
            fs::create_dir_all(p)?;
        }
        let mut key = vec![0; 32];
        rand_bytes(&mut key).map_err(|e| anyhow!("Failed to generate random key: {}", e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&key_path)?;
            file.write_all(&key)?;
        }
        #[cfg(windows)]
        {
            fs::write(&key_path, &key)?;
            let _ = Command::new("icacls")
                .arg(&key_path)
                .arg("/inheritance:r")
                .arg("/grant:r")
                .arg("*S-1-5-18:(F)")
                .arg("/grant:r")
                .arg("*S-1-5-32-544:(F)")
                .status();
        }
        #[cfg(not(any(unix, windows)))]
        {
            fs::write(&key_path, &key)?;
        }
    }
    let key = fs::read(&key_path)?;
    if key.len() != 32 {
        return Err(anyhow!("Invalid machine key length"));
    }
    Ok(key)
}

pub fn encrypt_password(phrase: &str) -> Result<String> {
    if !utils::is_admin() {
        return Err(anyhow!(
            "Administrator privileges required to access the machine key."
        ));
    }
    let key = get_machine_key()?;
    let mut iv = vec![0; 16];
    rand_bytes(&mut iv).map_err(|e| anyhow!("Failed to generate IV: {}", e))?;

    let cipher = Cipher::aes_256_cbc();
    let ciphertext = encrypt(cipher, &key, Some(&iv), phrase.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    Ok(format!("{}:{}", hex::encode(iv), hex::encode(ciphertext)))
}

pub fn decrypt_password(encrypted: &str) -> Result<String> {
    let parts: Vec<&str> = encrypted.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid encrypted password format"));
    }
    let iv = hex::decode(parts[0])?;
    let ciphertext = hex::decode(parts[1])?;
    let key = get_machine_key()?;

    let cipher = Cipher::aes_256_cbc();
    let decrypted = decrypt(cipher, &key, Some(&iv), &ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;

    Ok(String::from_utf8(decrypted)?)
}

pub fn get_system_config_lua_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\ProgramData\zoi\zoi.lua")
    } else {
        PathBuf::from("/etc/zoi/zoi.lua")
    }
}

pub fn parse_system_config_file(path: &Path) -> Result<types::DeclarativeConfig> {
    if !path.exists() {
        return Err(anyhow!(
            "System configuration file not found at: {}",
            path.display()
        ));
    }

    let lua = Lua::new();
    let sys_config_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiSystemConfig", sys_config_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    let platform = utils::get_platform()?;
    lua::functions::setup_lua_environment(
        &lua,
        &platform,
        None,
        Some(
            path.to_str()
                .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", path))?,
        ),
        None,
        None,
        true,
    )
    .map_err(|e| anyhow!("Failed to setup Lua environment: {}", e))?;

    let lua_code = fs::read_to_string(path)?;
    lua.load(&lua_code).exec().map_err(|e| {
        anyhow!(
            "Failed to execute system config at {}: {}",
            path.display(),
            e
        )
    })?;

    let final_table: Table = lua
        .globals()
        .get("__ZoiSystemConfig")
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut config: types::DeclarativeConfig = lua
        .from_value(Value::Table(final_table))
        .map_err(|e| anyhow!("Failed to parse system config: {}", e))?;

    for import_path in config.imports.clone() {
        let abs_import_path = if Path::new(&import_path).is_absolute() {
            PathBuf::from(import_path)
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new(""))
                .join(import_path)
        };

        let imported_config = parse_system_config_file(&abs_import_path)?;

        config.packages.extend(imported_config.packages);
        config.extensions.extend(imported_config.extensions);
        config.services.extend(imported_config.services);
        config.files.extend(imported_config.files);
        config.users.extend(imported_config.users);
        config.groups.extend(imported_config.groups);
        config.env.extend(imported_config.env);
        config.aliases.extend(imported_config.aliases);
        config.programs.extend(imported_config.programs);
    }

    config.packages.sort();
    config.packages.dedup();
    config.extensions.sort();
    config.extensions.dedup();
    config.services.sort();
    config.services.dedup();

    Ok(config)
}

pub fn parse_system_config() -> Result<types::DeclarativeConfig> {
    parse_system_config_file(&get_system_config_lua_path())
}

pub fn apply(yes: bool, plugin_manager: &crate::pkg::plugin::PluginManager) -> Result<()> {
    let is_arch = if cfg!(target_os = "linux") {
        utils::get_linux_distro_family() == Some("arch".to_string())
    } else {
        false
    };

    if !is_arch {
        return Err(anyhow!(
            "The declarative system configuration (zoi.lua) is currently only supported on Arch Linux systems."
        ));
    }

    if !utils::is_admin() {
        return Err(anyhow!(
            "Administrator privileges required to apply system configuration."
        ));
    }

    println!(
        "{} Applying Declarative System Configuration...",
        "::".bold().blue()
    );

    let config = parse_system_config()?;

    if let Some(boot) = &config.boot {
        apply_boot(boot)?;
    }

    if let Some(hw) = &config.hardware {
        apply_hardware(hw, yes, plugin_manager)?;
    }

    if let Some(hostname) = &config.hostname {
        apply_hostname(hostname)?;
    }

    if let Some(locale) = &config.locale {
        apply_locale(locale)?;
    }
    if let Some(timezone) = &config.timezone {
        apply_timezone(timezone)?;
    }

    if let Some(network) = &config.network {
        apply_network(network, yes, plugin_manager)?;
    }

    if let Some(de) = &config.desktop {
        apply_desktop(de, yes, plugin_manager)?;
    }

    if !config.groups.is_empty() {
        apply_groups(&config.groups)?;
    }

    if !config.users.is_empty() {
        apply_users(&config.users)?;
    }

    if let Some(shell) = &config.shell {
        apply_shell(shell)?;
    }

    if !config.packages.is_empty() {
        apply_packages(&config.packages, yes, plugin_manager)?;
    }

    if !config.extensions.is_empty() {
        apply_extensions(&config.extensions, yes, plugin_manager)?;
    }

    if !config.programs.is_empty() {
        apply_programs(&config.programs)?;
    }

    if !config.files.is_empty() {
        apply_files(&config.files)?;
    }

    if !config.env.is_empty() || !config.aliases.is_empty() {
        apply_env_aliases(&config.env, &config.aliases)?;
    }

    if !config.services.is_empty() {
        apply_services(&config.services)?;
    }

    println!(
        "\n{}",
        "✅ System configuration applied successfully!"
            .bold()
            .green()
    );
    Ok(())
}

fn apply_boot(boot: &types::BootConfig) -> Result<()> {
    println!("\n{}", ":: Synchronizing bootloader...".bold().blue());
    if !boot.kernel_params.is_empty() {
        println!(
            "Setting kernel parameters: {}",
            boot.kernel_params.join(" ").cyan()
        );
        let grub_path = Path::new("/etc/default/grub");
        if grub_path.exists() {
            let content = fs::read_to_string(grub_path)?;
            let mut new_content = String::new();
            for line in content.lines() {
                if line.starts_with("GRUB_CMDLINE_LINUX_DEFAULT=") {
                    new_content.push_str(&format!(
                        "GRUB_CMDLINE_LINUX_DEFAULT=\"{}\"\n",
                        boot.kernel_params.join(" ")
                    ));
                } else {
                    new_content.push_str(line);
                    new_content.push('\n');
                }
            }
            fs::write(grub_path, new_content)?;
            Command::new("grub-mkconfig")
                .arg("-o")
                .arg("/boot/grub/grub.cfg")
                .status()?;
        }
    }
    Ok(())
}

fn apply_hardware(
    hw: &types::HardwareConfig,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("\n{}", ":: Synchronizing hardware...".bold().blue());
    let mut pkgs = Vec::new();

    if let Some(microcode) = &hw.microcode {
        match microcode.to_lowercase().as_str() {
            "amd" => pkgs.push("native:amd-ucode".to_string()),
            "intel" => pkgs.push("native:intel-ucode".to_string()),
            _ => eprintln!("{}: Unknown microcode '{}'", "Warning".yellow(), microcode),
        }
    }

    for driver in &hw.drivers {
        match driver.to_lowercase().as_str() {
            "nvidia" => pkgs.push("native:nvidia".to_string()),
            "amdgpu" => {
                pkgs.push("native:xf86-video-amdgpu".to_string());
                pkgs.push("native:vulkan-radeon".to_string());
            }
            _ => eprintln!("{}: Unknown driver '{}'", "Warning".yellow(), driver),
        }
    }

    if !pkgs.is_empty() {
        let pkgs_ref: Vec<String> = pkgs.iter().map(|s| s.to_string()).collect();
        apply_packages(&pkgs_ref, yes, plugin_manager)?;
    }
    Ok(())
}

fn apply_network(
    net: &types::NetworkConfig,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("\n{}", ":: Synchronizing network...".bold().blue());

    if let Some(manager) = &net.manager {
        let mut pkgs = Vec::new();
        if manager.to_lowercase() == "networkmanager" {
            pkgs.push("native:networkmanager".to_string());
        }
        if !pkgs.is_empty() {
            let pkgs_ref: Vec<String> = pkgs.iter().map(|s| s.to_string()).collect();
            apply_packages(&pkgs_ref, yes, plugin_manager)?;
        }

        let svc = if manager.to_lowercase() == "networkmanager" {
            "NetworkManager"
        } else {
            manager
        };
        let _ = service::manage_service(svc, service::ServiceAction::Start);
    }

    if let Some(fw) = &net.firewall
        && fw.enable
    {
        apply_packages(&["native:ufw".to_string()], yes, plugin_manager)?;
        let _ = service::manage_service("ufw", service::ServiceAction::Start);

        Command::new("ufw").arg("--force").arg("enable").status()?;

        for port in &fw.allowed_tcp_ports {
            Command::new("ufw")
                .arg("allow")
                .arg(format!("{}/tcp", port))
                .status()?;
        }
        for port in &fw.allowed_udp_ports {
            Command::new("ufw")
                .arg("allow")
                .arg(format!("{}/udp", port))
                .status()?;
        }
    }

    Ok(())
}

fn apply_programs(programs: &HashMap<String, serde_json::Value>) -> Result<()> {
    println!("\n{}", ":: Synchronizing programs...".bold().blue());
    let home = get_real_user_home();

    for (prog_name, config) in programs {
        println!("Configuring program: {}", prog_name.cyan());

        match prog_name.as_str() {
            "git" => {
                if let Some(obj) = config.as_object() {
                    let mut gitconfig = String::new();
                    if let Some(user_name) = obj.get("userName").and_then(|v| v.as_str()) {
                        gitconfig.push_str(&format!("[user]\n\tname = {}\n", user_name));
                    }
                    if let Some(user_email) = obj.get("userEmail").and_then(|v| v.as_str()) {
                        gitconfig.push_str(&format!("\temail = {}\n", user_email));
                    }
                    if !gitconfig.is_empty() {
                        let path = home.join(".gitconfig");
                        fs::write(&path, gitconfig)?;
                        crate::utils::set_path_owner(
                            &path,
                            &std::env::var("SUDO_USER").unwrap_or_default(),
                            "",
                        )?;
                    }
                }
            }
            _ => {
                eprintln!(
                    "{}: Zoi does not yet have a built-in abstraction for '{}'.",
                    "Warning".yellow(),
                    prog_name
                );
            }
        }
    }
    Ok(())
}

fn apply_groups(groups: &HashMap<String, types::GroupConfig>) -> Result<()> {
    println!("\n{}", ":: Synchronizing groups...".bold().blue());
    for (name, cfg) in groups {
        let exists = Command::new("getent")
            .arg("group")
            .arg(name)
            .status()?
            .success();
        if !exists {
            println!("Creating group: {}", name.cyan());
            let mut cmd = Command::new("groupadd");
            if let Some(gid) = cfg.gid {
                cmd.arg("-g").arg(gid.to_string());
            }
            cmd.arg(name);
            cmd.status()?;
        }
    }
    Ok(())
}

fn apply_users(users: &HashMap<String, types::UserConfig>) -> Result<()> {
    println!("\n{}", ":: Synchronizing users...".bold().blue());
    for (name, cfg) in users {
        let exists = Command::new("id")
            .arg(name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success();

        if !exists {
            println!("Creating user: {}", name.cyan());
            let mut cmd = Command::new("useradd");
            cmd.arg("-m");

            if let Some(shell) = &cfg.shell {
                cmd.arg("-s").arg(shell);
            }
            if let Some(home) = &cfg.home {
                cmd.arg("-d").arg(home);
            }
            if let Some(groups) = &cfg.groups {
                cmd.arg("-G").arg(groups.join(","));
            }
            cmd.arg(name);
            cmd.status()?;
        } else {
            let mut cmd = Command::new("usermod");
            let mut modified = false;
            if let Some(shell) = &cfg.shell {
                cmd.arg("-s").arg(shell);
                modified = true;
            }
            if let Some(home) = &cfg.home {
                cmd.arg("-d").arg(home);
                modified = true;
            }
            if let Some(groups) = &cfg.groups {
                cmd.arg("-G").arg(groups.join(","));
                modified = true;
            }
            cmd.arg(name);
            if modified {
                println!("Updating user: {}", name.cyan());
                cmd.status()?;
            }
        }

        if let Some(enc_pass) = &cfg.password {
            println!("Setting password for user: {}", name.cyan());
            match decrypt_password(enc_pass) {
                Ok(plain_pass) => {
                    let mut child = Command::new("chpasswd").stdin(Stdio::piped()).spawn()?;
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin.write_all(format!("{}:{}", name, plain_pass).as_bytes())?;
                    }
                    child.wait()?;
                }
                Err(e) => {
                    eprintln!(
                        "{}: Failed to decrypt password for user '{}': {}",
                        "Warning".yellow(),
                        name,
                        e
                    );
                }
            }
        }
    }
    Ok(())
}

fn apply_hostname(hostname: &str) -> Result<()> {
    println!("Setting hostname to: {}", hostname.cyan());
    fs::write("/etc/hostname", hostname)?;
    let _ = Command::new("hostname").arg(hostname).status();
    Ok(())
}

fn apply_locale(locale: &str) -> Result<()> {
    println!("Setting locale to: {}", locale.cyan());
    let locale_gen_path = Path::new("/etc/locale.gen");
    if locale_gen_path.exists() {
        let content = fs::read_to_string(locale_gen_path)?;
        if !content.contains(&format!("{}.UTF-8 UTF-8", locale)) {
            let mut new_content = content;
            new_content.push_str(&format!("\n{}.UTF-8 UTF-8\n", locale));
            fs::write(locale_gen_path, new_content)?;
            println!("Running locale-gen...");
            Command::new("locale-gen").status()?;
        }
    }
    fs::write("/etc/locale.conf", format!("LANG={}.UTF-8\n", locale))?;
    Ok(())
}

fn apply_timezone(timezone: &str) -> Result<()> {
    println!("Setting timezone to: {}", timezone.cyan());
    let tz_path = PathBuf::from("/usr/share/zoneinfo").join(timezone);
    if tz_path.exists() {
        let localtime = Path::new("/etc/localtime");
        if localtime.exists() {
            fs::remove_file(localtime)?;
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(tz_path, localtime)?;
        }
        #[cfg(not(unix))]
        {
            return Err(anyhow!(
                "Timezone management is only supported on Unix-like systems."
            ));
        }
    } else {
        return Err(anyhow!("Timezone '{}' not found.", timezone));
    }
    Ok(())
}

fn apply_desktop(
    de: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("Ensuring desktop environment: {}", de.cyan());
    let pkgs = match de.to_lowercase().as_str() {
        "kde" | "plasma" => vec!["native:plasma-meta", "native:kde-applications-meta"],
        "gnome" => vec!["native:gnome", "native:gnome-extra"],
        "xfce" => vec!["native:xfce4", "native:xfce4-goodies"],
        "sway" => vec![
            "native:sway",
            "native:swaylock",
            "native:swayidle",
            "native:waybar",
        ],
        "hyprland" => vec![
            "native:hyprland",
            "native:xdg-desktop-portal-hyprland",
            "native:kitty",
        ],
        _ => return Err(anyhow!("Unsupported desktop environment: {}", de)),
    };
    let pkgs_string: Vec<String> = pkgs.iter().map(|s| s.to_string()).collect();
    apply_packages(&pkgs_string, yes, plugin_manager)
}

fn apply_shell(shell: &str) -> Result<()> {
    println!("Ensuring shell: {}", shell.cyan());
    let shell_path = format!("/bin/{}", shell);
    if !Path::new(&shell_path).exists() {
        return Err(anyhow!(
            "Shell '{}' not found at {}. Please include it in packages.",
            shell,
            shell_path
        ));
    }

    let user = std::env::var("SUDO_USER").unwrap_or_else(|_| "root".to_string());
    println!("Changing shell for user '{}' to {}", user, shell_path);
    Command::new("chsh")
        .arg("-s")
        .arg(&shell_path)
        .arg(&user)
        .status()?;
    Ok(())
}

fn apply_packages(
    packages: &[String],
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("\n{}", ":: Synchronizing packages...".bold().blue());
    let installed = local::get_installed_packages()?;
    let mut to_install = Vec::new();

    for pkg_source in packages {
        let req = crate::pkg::resolve::parse_source_string(pkg_source)?;
        let is_installed = installed.iter().any(|m| m.name == req.name);

        if !is_installed {
            to_install.push(pkg_source.clone());
        }
    }

    if !to_install.is_empty() {
        println!("Installing: {}", to_install.join(", ").cyan());
        crate::cmd::install::run(
            &to_install,
            None,
            false,
            true,
            yes,
            Some(crate::cli::InstallScope::System),
            false,
            false,
            false,
            None,
            false,
            plugin_manager,
            false,
        )?;
    } else {
        println!("All packages are already installed.");
    }
    Ok(())
}

fn apply_extensions(
    extensions: &[String],
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("\n{}", ":: Synchronizing extensions...".bold().blue());
    let installed = local::get_installed_packages()?;

    for ext_name in extensions {
        let is_added = installed
            .iter()
            .any(|m| m.name == *ext_name && m.package_type == types::PackageType::Extension);
        if !is_added {
            println!("Adding extension: {}", ext_name.cyan());
            extension::add(ext_name, yes, plugin_manager)?;
        }
    }
    Ok(())
}

fn get_real_user_home() -> PathBuf {
    if let Ok(user) = std::env::var("SUDO_USER") {
        PathBuf::from(format!("/home/{}", user))
    } else {
        home::home_dir().unwrap_or_else(|| PathBuf::from("/root"))
    }
}

fn apply_files(files: &HashMap<String, types::FileConfig>) -> Result<()> {
    println!("\n{}", ":: Synchronizing files...".bold().blue());
    let home = get_real_user_home();

    for (path_str, file_cfg) in files {
        let path = PathBuf::from(path_str.replace("~", &home.to_string_lossy()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if let Some(content) = &file_cfg.content {
            println!("Writing file: {}", path.display());
            fs::write(&path, content)?;
        } else if let Some(source) = &file_cfg.source {
            if source.starts_with("http") {
                println!("Downloading file {} to {}...", source, path.display());
                let resp = reqwest::blocking::get(source)?;
                fs::write(&path, resp.bytes()?)?;
            } else {
                let src_path = Path::new(source);
                if src_path.exists() {
                    println!("Copying {} to {}...", source, path.display());
                    fs::copy(src_path, &path)?;
                }
            }
        }

        if let Some(true) = file_cfg.executable {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(perms.mode() | 0o111);
                fs::set_permissions(&path, perms)?;
            }
        }

        if let Some(owner) = &file_cfg.owner {
            let group = file_cfg.group.as_deref().unwrap_or("");
            crate::utils::set_path_owner(&path, owner, group)?;
        }

        if let Some(_mode) = file_cfg.mode {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(_mode))?;
            }
        }
    }
    Ok(())
}

fn apply_env_aliases(
    env: &HashMap<String, String>,
    aliases: &HashMap<String, String>,
) -> Result<()> {
    println!(
        "\n{}",
        ":: Synchronizing environment and aliases...".bold().blue()
    );

    let zoi_profile_path = Path::new("/etc/profile.d/zoi_declarative.sh");
    let mut content = String::from("# Generated by Zoi declarative system configuration\n\n");

    for (k, v) in env {
        content.push_str(&format!("export {}=\"{}\"\n", k, v));
    }

    for (k, v) in aliases {
        content.push_str(&format!("alias {}='{}'\n", k, v));
    }

    fs::write(zoi_profile_path, content)?;
    println!("Updated {}", zoi_profile_path.display());
    Ok(())
}

fn apply_services(services: &[String]) -> Result<()> {
    println!("\n{}", ":: Synchronizing services...".bold().blue());
    for svc_name in services {
        println!("Ensuring service is running: {}", svc_name.cyan());
        if let Err(e) = service::manage_service(svc_name, service::ServiceAction::Start) {
            eprintln!(
                "{}: Failed to start service '{}': {}",
                "Warning".yellow(),
                svc_name,
                e
            );
        }
    }
    Ok(())
}
