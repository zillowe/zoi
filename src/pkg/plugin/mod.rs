use crate::pkg::{local, resolve};
use crate::project;
use anyhow::{Result, anyhow};
use colored::*;
use comfy_table::{Table as ComfyTable, presets::UTF8_FULL};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct PluginManager {
    pub lua: Lua,
}

impl PluginManager {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        let manager = Self { lua };
        manager.setup_api()?;
        Ok(manager)
    }

    fn setup_api(&self) -> Result<()> {
        let zoi = self
            .lua
            .create_table()
            .map_err(|e| anyhow!(e.to_string()))?;

        self.lua
            .globals()
            .set(
                "__ZOI_COMMANDS",
                self.lua
                    .create_table()
                    .map_err(|e| anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow!(e.to_string()))?;
        self.lua
            .globals()
            .set(
                "__ZOI_COMMAND_HELP",
                self.lua
                    .create_table()
                    .map_err(|e| anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow!(e.to_string()))?;

        let register_command = self.lua.create_function(|lua, arg: Value| {
            let registry: Table = lua.globals().get("__ZOI_COMMANDS")?;
            let help_registry: Table = lua.globals().get("__ZOI_COMMAND_HELP")?;
            match arg {
                Value::Table(t) => {
                    let name: String = t.get("name")?;
                    let desc: String = t.get("description").unwrap_or_else(|_| "".to_string());
                    let callback: Function = t.get("callback")?;
                    registry.set(name.clone(), callback)?;
                    help_registry.set(name, desc)?;
                },
                _ => return Err(mlua::Error::RuntimeError("Invalid argument to register_command. Expected a table {name, description, callback}".to_string())),
            }
            Ok(())
        }).map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("register_command", register_command)
            .map_err(|e| anyhow!(e.to_string()))?;

        let register_command_simple = self
            .lua
            .create_function(|lua, (name, callback): (String, Function)| {
                let registry: Table = lua.globals().get("__ZOI_COMMANDS")?;
                registry.set(name, callback)?;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("register_command_simple", register_command_simple)
            .map_err(|e| anyhow!(e.to_string()))?;

        self.lua
            .globals()
            .set(
                "__ZOI_HOOKS",
                self.lua
                    .create_table()
                    .map_err(|e| anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow!(e.to_string()))?;
        let hooks = [
            "on_pre_install",
            "on_post_install",
            "on_pre_uninstall",
            "on_post_uninstall",
            "on_pre_sync",
            "on_post_sync",
            "on_rollback",
            "on_pre_create",
            "on_post_create",
            "on_pre_extension_add",
            "on_post_extension_add",
            "on_pre_extension_remove",
            "on_post_extension_remove",
        ];
        for hook in hooks {
            let hook_name = hook.to_string();
            let register_hook = self
                .lua
                .create_function(move |lua, callback: Function| {
                    let registry: Table = lua.globals().get("__ZOI_HOOKS")?;
                    let hook_list: Table = match registry.get(hook_name.as_str()) {
                        Ok(t) => t,
                        Err(_) => {
                            let t = lua.create_table()?;
                            registry.set(hook_name.as_str(), t.clone())?;
                            t
                        }
                    };
                    hook_list.push(callback)?;
                    Ok(())
                })
                .map_err(|e| anyhow!(e.to_string()))?;
            zoi.set(hook, register_hook)
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        let set_data = self
            .lua
            .create_function(|_, (key, value): (String, Value)| {
                let mut state = read_plugin_state().unwrap_or_default();
                let json_val: serde_json::Value = match value {
                    Value::String(s) => serde_json::Value::String(s.to_str()?.to_string()),
                    Value::Integer(i) => serde_json::Value::Number(i.into()),
                    Value::Number(n) => {
                        serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap())
                    }
                    Value::Boolean(b) => serde_json::Value::Bool(b),
                    _ => {
                        return Err(mlua::Error::RuntimeError(
                            "Unsupported value type for set_data".to_string(),
                        ));
                    }
                };
                state.insert(key, json_val);
                write_plugin_state(&state).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("set_data", set_data)
            .map_err(|e| anyhow!(e.to_string()))?;

        let get_data = self
            .lua
            .create_function(|lua, key: String| {
                let state = read_plugin_state().unwrap_or_default();
                if let Some(val) = state.get(&key) {
                    lua.to_value(val)
                } else {
                    Ok(Value::Nil)
                }
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("get_data", get_data)
            .map_err(|e| anyhow!(e.to_string()))?;

        let list_installed = self
            .lua
            .create_function(|lua, _: ()| {
                let installed = local::get_installed_packages()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                lua.to_value(&installed)
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("list_installed", list_installed)
            .map_err(|e| anyhow!(e.to_string()))?;

        let get_package = self
            .lua
            .create_function(|lua, name: String| {
                let (pkg, _, _, _, _) = resolve::resolve_package_and_version(&name, true)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                lua.to_value(&pkg)
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("get_package", get_package)
            .map_err(|e| anyhow!(e.to_string()))?;

        if let Ok(config) = project::config::load() {
            let project_table = self
                .lua
                .create_table()
                .map_err(|e| anyhow!(e.to_string()))?;
            project_table
                .set("name", config.name)
                .map_err(|e| anyhow!(e.to_string()))?;
            project_table
                .set("packages", config.pkgs)
                .map_err(|e| anyhow!(e.to_string()))?;
            zoi.set("project", project_table)
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        let ui = self
            .lua
            .create_table()
            .map_err(|e| anyhow!(e.to_string()))?;
        let ui_print = self
            .lua
            .create_function(|_, (text, color): (String, Option<String>)| {
                let colored_text = match color.as_deref() {
                    Some("red") => text.red(),
                    Some("green") => text.green(),
                    Some("yellow") => text.yellow(),
                    Some("blue") => text.blue(),
                    Some("cyan") => text.cyan(),
                    Some("magenta") => text.magenta(),
                    _ => text.normal(),
                };
                println!("{}", colored_text);
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        ui.set("print", ui_print)
            .map_err(|e| anyhow!(e.to_string()))?;

        let ui_confirm = self
            .lua
            .create_function(|_, prompt: String| {
                Ok(Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(prompt)
                    .interact()
                    .unwrap_or(false))
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        ui.set("confirm", ui_confirm)
            .map_err(|e| anyhow!(e.to_string()))?;

        let ui_select = self
            .lua
            .create_function(|_, (prompt, options): (String, Vec<String>)| {
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt(prompt)
                    .items(&options)
                    .default(0)
                    .interact_opt()
                    .unwrap_or(None);
                Ok(selection.map(|s| s + 1))
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        ui.set("select", ui_select)
            .map_err(|e| anyhow!(e.to_string()))?;

        let ui_table = self
            .lua
            .create_function(|_, (headers, rows): (Vec<String>, Vec<Vec<String>>)| {
                let mut table = ComfyTable::new();
                table.load_preset(UTF8_FULL).set_header(headers);
                for row in rows {
                    table.add_row(row);
                }
                println!("{}", table);
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        ui.set("table", ui_table)
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("ui", ui).map_err(|e| anyhow!(e.to_string()))?;

        let system = self
            .lua
            .create_table()
            .map_err(|e| anyhow!(e.to_string()))?;
        let platform =
            crate::utils::get_platform().unwrap_or_else(|_| "unknown-unknown".to_string());
        let parts: Vec<&str> = platform.split('-').collect();
        system
            .set("os", parts.first().unwrap_or(&"unknown").to_string())
            .map_err(|e| anyhow!(e.to_string()))?;
        system
            .set("arch", parts.get(1).unwrap_or(&"unknown").to_string())
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("system", system)
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("version", env!("CARGO_PKG_VERSION"))
            .map_err(|e| anyhow!(e.to_string()))?;

        let shell = self
            .lua
            .create_function(|_, cmd: String| {
                let status = if cfg!(target_os = "windows") {
                    std::process::Command::new("pwsh")
                        .arg("-Command")
                        .arg(&cmd)
                        .status()
                } else {
                    std::process::Command::new("bash")
                        .arg("-c")
                        .arg(&cmd)
                        .status()
                };
                match status {
                    Ok(s) => Ok(s.code().unwrap_or(if s.success() { 0 } else { 1 })),
                    Err(e) => Err(mlua::Error::RuntimeError(e.to_string())),
                }
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        zoi.set("sh", shell).map_err(|e| anyhow!(e.to_string()))?;

        self.lua
            .globals()
            .set("zoi", zoi)
            .map_err(|e| anyhow!(e.to_string()))?;

        let plugin_dir = get_plugin_dir()?;
        let import_fn = self
            .lua
            .create_function(move |lua, file_name: String| {
                let path = plugin_dir.join(&file_name);
                if !path.exists() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "File not found: {}",
                        path.display()
                    )));
                }
                let content = fs::read_to_string(&path)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    match ext {
                        "json" => {
                            let val: serde_json::Value = serde_json::from_str(&content)
                                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                            return lua.to_value(&val);
                        }
                        _ => return lua.to_value(&content),
                    }
                }
                lua.to_value(&content)
            })
            .map_err(|e| anyhow!(e.to_string()))?;
        self.lua
            .globals()
            .set("IMPORT", import_fn)
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(())
    }

    pub fn load_all(&self) -> Result<()> {
        let plugin_dir = get_plugin_dir()?;
        if !plugin_dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(plugin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                let script = fs::read_to_string(&path)?;
                let script = format!(
                    "local old_reg = zoi.register_command; zoi.register_command = function(a, b) if type(a) == 'string' then zoi.register_command_simple(a, b) else old_reg(a) end end; {}",
                    script
                );
                self.lua
                    .load(&script)
                    .exec()
                    .map_err(|e| anyhow!("Plugin error in {}: {}", path.display(), e))?;
            }
        }
        Ok(())
    }

    pub fn trigger_hook(&self, hook_name: &str, arg: Option<Value>) -> Result<()> {
        let registry: Table = self
            .lua
            .globals()
            .get("__ZOI_HOOKS")
            .map_err(|e| anyhow!(e.to_string()))?;
        if let Ok(hook_list) = registry.get::<Table>(hook_name) {
            for callback in hook_list.sequence_values::<Function>() {
                let callback = callback.map_err(|e| anyhow!(e.to_string()))?;
                if let Some(a) = &arg {
                    callback
                        .call::<()>(a.clone())
                        .map_err(|e| anyhow!(e.to_string()))?;
                } else {
                    callback
                        .call::<()>(())
                        .map_err(|e| anyhow!(e.to_string()))?;
                }
            }
        }
        Ok(())
    }

    pub fn run_command(&self, name: &str, args: Vec<String>) -> Result<bool> {
        let registry: Table = self
            .lua
            .globals()
            .get("__ZOI_COMMANDS")
            .map_err(|e| anyhow!(e.to_string()))?;
        let callback: Value = registry.get(name).map_err(|e| anyhow!(e.to_string()))?;
        if let Value::Function(func) = callback {
            func.call::<()>(args).map_err(|e| anyhow!(e.to_string()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list_commands(&self) -> Result<Vec<(String, String)>> {
        let registry: Table = self
            .lua
            .globals()
            .get("__ZOI_COMMANDS")
            .map_err(|e| anyhow!(e.to_string()))?;
        let help_registry: Table = self
            .lua
            .globals()
            .get("__ZOI_COMMAND_HELP")
            .map_err(|e| anyhow!(e.to_string()))?;
        let mut commands = Vec::new();
        for pair in registry.pairs::<String, Value>() {
            let (name, _) = pair.map_err(|e| anyhow!(e.to_string()))?;
            let desc: String = help_registry
                .get(name.clone())
                .unwrap_or_else(|_| "".to_string());
            commands.push((name, desc));
        }
        Ok(commands)
    }
}

pub fn get_plugin_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let plugin_dir = home_dir.join(".zoi").join("plugins");
    if !plugin_dir.exists() {
        fs::create_dir_all(&plugin_dir)?;
    }
    Ok(plugin_dir)
}

fn read_plugin_state() -> Result<HashMap<String, serde_json::Value>> {
    let path = get_plugin_dir()?.join("state.json");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

fn write_plugin_state(state: &HashMap<String, serde_json::Value>) -> Result<()> {
    let path = get_plugin_dir()?.join("state.json");
    let content = serde_json::to_string_pretty(state)?;
    fs::write(path, content)?;
    Ok(())
}
