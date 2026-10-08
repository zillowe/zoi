use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// The `system({...})` block: identity and locale settings.
pub struct SystemMetadata {
    /// System hostname.
    pub hostname: Option<String>,
    /// System timezone.
    pub timezone: Option<String>,
    /// System locale.
    pub locale: Option<String>,
    /// Parameters appended to the kernel command line.
    pub kernel_params: Option<String>,
    /// Desktop environment to provision.
    pub desktop: Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// The `bootloader({...})` block.
pub struct BootloaderConfig {
    #[serde(rename = "type")]
    /// Bootloader implementation to use.
    pub boot_type: String, // "grub2", "systemd-boot", "limine"
    /// EFI system partition mount point.
    pub efi_dir: Option<String>,
    /// Boot menu timeout in seconds.
    pub timeout: Option<u32>
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// One entry of the `users({...})` block.
pub struct UserConfig {
    /// One-way hash of the account password.
    pub password_hash: Option<String>,
    /// Supplementary groups.
    pub groups: Option<Vec<String>>,
    /// Login shell path.
    pub shell: Option<String>,
    /// Home directory path.
    pub home: Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// One entry of the `groups({...})` block.
pub struct GroupConfig {
    /// Numeric group id.
    pub gid: Option<u32>
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// One entry of the `services({...})` block.
pub struct ServiceConfig {
    /// Whether the unit is enabled.
    pub enable: bool
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// One entry of the `filesystems({...})` block.
pub struct FilesystemConfig {
    /// Block device or label to mount.
    pub device: String,
    /// Mount point.
    pub mount: String,
    #[serde(rename = "type")]
    /// Filesystem type.
    pub fs_type: String,
    /// Options selected by the administrator.
    pub options: Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// A fully parsed `system.lua`.
pub struct SystemConfig {
    /// The `system({...})` block.
    pub system: SystemMetadata,
    /// The `bootloader({...})` block.
    pub bootloader: Option<BootloaderConfig>,
    /// Packages this record refers to.
    pub packages: Vec<String>,
    /// Packages v2.
    pub packages_v2: HashMap<String, zoi_project::config::PackageSpec>,
    /// User accounts.
    pub users: HashMap<String, UserConfig>,
    /// Supplementary groups.
    pub groups: HashMap<String, GroupConfig>,
    /// The `services({...})` block.
    pub services: HashMap<String, ServiceConfig>,
    /// The `filesystems({...})` block.
    pub filesystems: Vec<FilesystemConfig>
}

/// Parses a `system.lua` file.
///
/// # Errors
///
/// Returns an error if the file cannot be read, is not valid Lua, or does
/// not evaluate to the shape `system.lua` is expected to produce.
pub fn load_system_lua<P: AsRef<Path>>(path: P) -> Result<SystemConfig> {
    let lua = Lua::new();
    let content = fs::read_to_string(path)?;

    let system_data =
        std::sync::Arc::new(std::sync::Mutex::new(SystemMetadata::default()));
    let bootloader_data = std::sync::Arc::new(std::sync::Mutex::new(None));
    let packages_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let packages_v2_data =
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let users_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let groups_data =
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let services_data =
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let filesystems_data =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    // Define 'system' function
    let s_clone = system_data.clone();
    let system_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = s_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            *data = lua
                .from_value(Value::Table(table))
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("system", system_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'bootloader' function
    let b_clone = bootloader_data.clone();
    let bootloader_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = b_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            *data = Some(
                lua.from_value(Value::Table(table))
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
            );
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("bootloader", bootloader_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'packages' function
    let p_clone = packages_data.clone();
    let pv2_clone = packages_v2_data.clone();
    let packages_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = p_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            let mut data_v2 = pv2_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<Value, Value>() {
                let (k, v) = pair?;
                match k {
                    Value::String(s) => {
                        let key = s.to_str()?.trim().to_string();
                        let spec = lua
                            .from_value::<zoi_project::config::PackageSpec>(
                                v
                            )?;
                        data_v2.insert(key.clone(), spec.clone());
                        let mut ident = key;
                        if let Some(ver) = &spec.version {
                            if ver.starts_with('@') {
                                ident = format!("{ident}{ver}");
                            } else {
                                ident = format!("{ident}@{ver}");
                            }
                        }
                        data.push(ident);
                    }
                    Value::Integer(_) => {
                        if let Value::String(s) = v {
                            data.push(s.to_str()?.trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("packages", packages_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'users' function
    let u_clone = users_data.clone();
    let users_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = u_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<String, Value>() {
                let (k, v) =
                    pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let spec = lua
                    .from_value::<UserConfig>(v)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.insert(k, spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("users", users_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'groups' function
    let g_clone = groups_data.clone();
    let groups_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = g_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<String, Value>() {
                let (k, v) =
                    pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let spec = lua
                    .from_value::<GroupConfig>(v)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.insert(k, spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("groups", groups_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'services' function
    let svc_clone = services_data.clone();
    let services_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = svc_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<String, Value>() {
                let (k, v) =
                    pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let spec = lua
                    .from_value::<ServiceConfig>(v)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.insert(k, spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("services", services_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'filesystems' function
    let fs_clone = filesystems_data.clone();
    let filesystems_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = fs_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for val in table.sequence_values::<Value>() {
                let spec = lua
                    .from_value::<FilesystemConfig>(val.map_err(|e| {
                        mlua::Error::RuntimeError(e.to_string())
                    })?)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.push(spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("filesystems", filesystems_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    lua.load(&content)
        .exec()
        .map_err(|e| anyhow!("Failed to execute system.lua: {e}"))?;

    Ok(SystemConfig {
        system: system_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        bootloader: bootloader_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        packages: packages_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        packages_v2: packages_v2_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        users: users_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        groups: groups_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        services: services_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        filesystems: filesystems_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone()
    })
}
