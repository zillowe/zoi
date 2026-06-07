use mlua::{self, Lua, LuaSerdeExt, Table};

pub fn add_parse_util(lua: &Lua) -> Result<(), mlua::Error> {
    let parse_table = lua.create_table()?;

    let json_fn = lua.create_function(|lua, json_str: String| {
        let value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("json", json_fn)?;

    let yaml_fn = lua.create_function(|lua, yaml_str: String| {
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("yaml", yaml_fn)?;

    let toml_fn = lua.create_function(|lua, toml_str: String| {
        let value: toml::Value =
            toml::from_str(&toml_str).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("toml", toml_fn)?;

    let checksum_fn = lua.create_function(|_, (content, file_name): (String, String)| {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 && parts[1] == file_name {
                return Ok(Some(parts[0].to_string()));
            }
        }
        Ok(None)
    })?;
    parse_table.set("checksumFile", checksum_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("PARSE", parse_table)?;

    Ok(())
}
