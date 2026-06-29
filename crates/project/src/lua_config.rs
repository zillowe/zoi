use crate::config::{
    CommandSpec, EnvironmentSpec, PackageSpec, ProjectConfig, ProjectLocalConfig, RegistrySpec,
};
use anyhow::{Result, anyhow};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn load_zoi_lua(path: &Path) -> Result<ProjectConfig> {
    let lua = Lua::new();
    let content = fs::read_to_string(path)?;

    let project_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let packages_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let registries_data = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
    let tasks_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let environments_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let env_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    for (k, v) in std::env::vars() {
        env_table.set(k, v).map_err(|e| anyhow!(e.to_string()))?;
    }
    lua.globals()
        .set("ENV", env_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    let p_clone = project_data.clone();
    let project_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = p_clone.lock().unwrap();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair?;
                data.insert(k, lua.from_value::<serde_json::Value>(v)?);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("project", project_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    let pkgs_clone = packages_data.clone();
    let packages_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = pkgs_clone.lock().unwrap();
            for pair in table.pairs::<Value, Value>() {
                let (k, v) = pair?;
                match k {
                    Value::String(s) => {
                        let key = s.to_str()?.to_string();
                        let spec = lua.from_value::<PackageSpec>(v)?;
                        data.insert(key, spec);
                    }
                    Value::Integer(_) => {
                        if let Value::String(s) = v {
                            data.insert(
                                s.to_str()?.to_string(),
                                PackageSpec {
                                    package_type: None,
                                    install_method: None,
                                    sub_packages: None,
                                    version: None,
                                    dependencies: None,
                                },
                            );
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

    let regs_clone = registries_data.clone();
    let registries_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = regs_clone.lock().unwrap();
            for pair in table.pairs::<String, Value>() {
                let (k, v) = pair?;
                let spec = lua.from_value::<RegistrySpec>(v)?;
                data.insert(k, spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("registries", registries_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    let tasks_clone = tasks_data.clone();
    let tasks_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = tasks_clone.lock().unwrap();
            for val in table.sequence_values::<Value>() {
                let spec = lua.from_value::<CommandSpec>(val?)?;
                data.push(spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("tasks", tasks_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    let envs_clone = environments_data.clone();
    let environments_fn = lua
        .create_function(move |lua, table: Table| {
            let mut data = envs_clone.lock().unwrap();
            for val in table.sequence_values::<Value>() {
                let spec = lua.from_value::<EnvironmentSpec>(val?)?;
                data.push(spec);
            }
            Ok(())
        })
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("environments", environments_fn)
        .map_err(|e| anyhow!(e.to_string()))?;

    lua.load(&content)
        .exec()
        .map_err(|e| anyhow!("Failed to execute zoi.lua: {}", e))?;

    let project_map = project_data.lock().unwrap();
    let name = project_map
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("zoi.lua must define project name"))?;

    let local = project_map
        .get("local")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut pkgs = Vec::new();
    let pkgs_v2 = packages_data.lock().unwrap().clone();
    for (k, spec) in &pkgs_v2 {
        let mut s = k.clone();
        if let Some(v) = &spec.version {
            s = format!("{}@{}", s, v);
        }
        pkgs.push(s);
    }

    Ok(ProjectConfig {
        name,
        registries: registries_data.lock().unwrap().clone(),
        packages: Vec::new(),
        pkgs,
        pkgs_v2,
        config: ProjectLocalConfig { local },
        commands: tasks_data.lock().unwrap().clone(),
        environments: environments_data.lock().unwrap().clone(),
        shell: None,
    })
}
