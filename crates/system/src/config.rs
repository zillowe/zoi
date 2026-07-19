use anyhow::{Result, anyhow};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SystemMetadata {
    pub hostname: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub kernel_params: Option<String>,
    pub desktop: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BootloaderConfig {
    #[serde(rename = "type")]
    pub boot_type: String, // "grub2", "systemd-boot", "limine"
    pub efi_dir: Option<String>,
    pub timeout: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserConfig {
    pub password_hash: Option<String>,
    pub groups: Option<Vec<String>>,
    pub shell: Option<String>,
    pub home: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupConfig {
    pub gid: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ServiceConfig {
    pub enable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilesystemConfig {
    pub device: String,
    pub mount: String,
    #[serde(rename = "type")]
    pub fs_type: String,
    pub options: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SystemConfig {
    pub system: SystemMetadata,
    pub bootloader: Option<BootloaderConfig>,
    pub packages: Vec<String>,
    pub users: HashMap<String, UserConfig>,
    pub groups: HashMap<String, GroupConfig>,
    pub services: HashMap<String, ServiceConfig>,
    pub filesystems: Vec<FilesystemConfig>,
}

pub fn load_system_lua<P: AsRef<Path>>(path: P) -> Result<SystemConfig> {
    let lua = Lua::new();
    let content = fs::read_to_string(path)?;

    let system_data = std::sync::Arc::new(std::sync::Mutex::new(SystemMetadata::default()));
    let bootloader_data = std::sync::Arc::new(std::sync::Mutex::new(None));
    let packages_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let users_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let groups_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let services_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let filesystems_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    // Define 'system' function
    let s_clone = system_data.clone();
    let system_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = s_clone.lock().unwrap();
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
            let mut data = b_clone.lock().unwrap();
            *data = Some(
                lua.from_value(Value::Table(table))
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?,
            );
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("bootloader", bootloader_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'packages' function
    let p_clone = packages_data.clone();
    let packages_fn = lua
        .create_function(move |_, table: Table| {
            let mut data = p_clone.lock().unwrap();
            for val in table.sequence_values::<String>() {
                data.push(val.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?);
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
            let mut data = u_clone.lock().unwrap();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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
            let mut data = g_clone.lock().unwrap();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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
            let mut data = svc_clone.lock().unwrap();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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
            let mut data = fs_clone.lock().unwrap();
            for val in table.sequence_values::<Value>() {
                let spec = lua
                    .from_value::<FilesystemConfig>(
                        val.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?,
                    )
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
        .map_err(|e| anyhow!("Failed to execute system.lua: {}", e))?;

    Ok(SystemConfig {
        system: system_data.lock().unwrap().clone(),
        bootloader: bootloader_data.lock().unwrap().clone(),
        packages: packages_data.lock().unwrap().clone(),
        users: users_data.lock().unwrap().clone(),
        groups: groups_data.lock().unwrap().clone(),
        services: services_data.lock().unwrap().clone(),
        filesystems: filesystems_data.lock().unwrap().clone(),
    })
}
