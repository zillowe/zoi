use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use mlua::{Lua, LuaSerdeExt, Table};
use serde::{Deserialize, Serialize};

/// Appends formatted output to a `String` without an intermediate allocation.
///
/// `write!` is used rather than `push_str(&format!(..))` so each secret is
/// formatted straight into the buffer. The result cannot fail, because the
/// sink is a `String`.
fn emit(content: &mut String, args: std::fmt::Arguments<'_>) {
    use std::fmt::Write as _;
    content
        .write_fmt(args)
        .expect("writing into a String cannot fail");
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
/// A fully parsed `home.lua`.
pub struct HomeConfig {
    /// Packages this record refers to.
    pub packages: Vec<String>,
    /// Packages v2.
    pub packages_v2: HashMap<String, zoi_project::config::PackageSpec>,
    /// The `dotfiles({...})` block: files copied or symlinked into the home
    /// directory.
    pub dotfiles: HashMap<String, String>,
    /// The `env({...})` block: variables written to the user's `env` file.
    pub env: HashMap<String, String>
}

/// Parses a `home.lua` file.
///
/// # Errors
///
/// Returns an error if the file cannot be read, is not valid Lua, or does
/// not evaluate to the shape `home.lua` is expected to produce.
pub fn load_home_lua<P: AsRef<Path>>(path: P) -> Result<HomeConfig> {
    let lua = Lua::new();
    let content = fs::read_to_string(path)?;

    let packages_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let packages_v2_data =
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let dotfiles_data =
        std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let env_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));

    // Define 'packages' function
    let p_clone = packages_data.clone();
    let pv2_clone = packages_v2_data.clone();
    let packages_fn = lua
        .create_function(move |lua, table: mlua::Table| {
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
            for pair in table.pairs::<mlua::Value, mlua::Value>() {
                let (k, v) = pair?;
                match k {
                    mlua::Value::String(s) => {
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
                    mlua::Value::Integer(_) => {
                        if let mlua::Value::String(s) = v {
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

    // Define 'dotfiles' function
    let d_clone = dotfiles_data.clone();
    let dotfiles_fn = lua
        .create_function(move |_, table: Table| {
            let mut data = d_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<String, String>() {
                let (k, v) =
                    pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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
            let mut data = e_clone.lock().map_err(|_| {
                mlua::Error::runtime(
                    "system.lua configuration state is poisoned"
                )
            })?;
            for pair in table.pairs::<String, String>() {
                let (k, v) =
                    pair.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
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
        .map_err(|e| anyhow!("Failed to execute home.lua: {e}"))?;

    Ok(HomeConfig {
        packages: packages_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        packages_v2: packages_v2_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        dotfiles: dotfiles_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone(),
        env: env_data
            .lock()
            .map_err(|_| anyhow!("system.lua configuration state is poisoned"))?
            .clone()
    })
}

/// Symlinks the configured dotfiles and writes the environment file.
///
/// # Errors
///
/// Returns an error if a dotfile symlink or the environment file cannot
/// be written, or if a declared secret cannot be decrypted.
pub fn apply_home_config(config: &HomeConfig) -> Result<()> {
    // Manage dotfiles symlinks
    let home_dir = zoi_core::utils::get_user_home()
        .ok_or_else(|| anyhow!("Could not find home directory"))?;

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
    let zoi_env_path = zoi_core::utils::get_user_config_dir()?.join("env");
    let mut env_content = String::from(
        "# Zoi Environment Variables\n# Generated from home.lua\n\n"
    );

    for (key, value) in &config.env {
        let decrypted_value = crate::secret::decrypt_secret(value)?;
        // Escape single quotes for shell safety
        let escaped_value = decrypted_value.replace('\'', "'\\''");
        emit(
            &mut env_content,
            format_args!("export {key}='{escaped_value}'\n")
        );
    }

    if let Some(parent) = zoi_env_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(zoi_env_path, env_content)?;

    Ok(())
}
