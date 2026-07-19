use anyhow::{Result, anyhow};
use mlua::{Lua, Table};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HomeConfig {
    pub packages: Vec<String>,
    pub dotfiles: HashMap<String, String>,
    pub env: HashMap<String, String>,
}

pub fn load_home_lua<P: AsRef<Path>>(path: P) -> Result<HomeConfig> {
    let lua = Lua::new();
    let content = fs::read_to_string(path)?;

    let packages_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let dotfiles_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let env_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));

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

    // Define 'dotfiles' function
    let d_clone = dotfiles_data.clone();
    let dotfiles_fn = lua
        .create_function(move |_, table: Table| {
            let mut data = d_clone.lock().unwrap();
            for pair in table.pairs::<String, String>() {
                let (k, v) = pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.insert(k, v);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("dotfiles", dotfiles_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Define 'env' function
    let e_clone = env_data.clone();
    let env_fn = lua
        .create_function(move |_, table: Table| {
            let mut data = e_clone.lock().unwrap();
            for pair in table.pairs::<String, String>() {
                let (k, v) = pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                data.insert(k, v);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("env", env_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    lua.load(&content)
        .exec()
        .map_err(|e| anyhow!("Failed to execute home.lua: {}", e))?;

    Ok(HomeConfig {
        packages: packages_data.lock().unwrap().clone(),
        dotfiles: dotfiles_data.lock().unwrap().clone(),
        env: env_data.lock().unwrap().clone(),
    })
}

pub fn apply_home_config(config: &HomeConfig) -> Result<()> {
    // Install user packages
    // This would call zoi::install_sources with Scope::User

    // Manage dotfiles symlinks
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

    for (target, source) in &config.dotfiles {
        let target_path = home_dir.join(target);

        // Decrypt source path if it's a secret
        let decrypted_source = crate::secret::decrypt_secret(source)?;
        let expanded_source = zoi_core::utils::expand_tilde(&decrypted_source);
        let source_path = Path::new(&expanded_source);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if target_path.exists() || target_path.is_symlink() {
            fs::remove_file(&target_path)?;
        }

        zoi_core::utils::symlink_file(source_path, &target_path)?;
    }

    // Apply Environment Variables
    let zoi_env_path = home_dir.join(".zoi/env");
    let mut env_content =
        String::from("# Zoi Environment Variables\n# Generated from home.lua\n\n");

    for (key, value) in &config.env {
        let decrypted_value = crate::secret::decrypt_secret(value)?;
        env_content.push_str(&format!("export {}='{}'\n", key, decrypted_value));
    }

    if let Some(parent) = zoi_env_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(zoi_env_path, env_content)?;

    Ok(())
}
