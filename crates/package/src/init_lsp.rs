use anyhow::{Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

const ZOI_LUA_DEFINITIONS: &str = include_str!("../../core/src/builtin/lsp/zoi.lua");

pub fn setup_lsp_workspace(path: &Path) -> Result<()> {
    let lsp_dir = get_lsp_definitions_dir()?;
    fs::create_dir_all(&lsp_dir)?;

    let zoi_lua_path = lsp_dir.join("zoi.lua");
    fs::write(&zoi_lua_path, ZOI_LUA_DEFINITIONS)?;

    let luarc_path = path.join(".luarc.json");
    if !luarc_path.exists() {
        let luarc_content = generate_luarc_json(&lsp_dir)?;
        fs::write(luarc_path, luarc_content)?;
    }

    Ok(())
}

fn get_lsp_definitions_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(home_dir.join(".zoi").join("lsp"))
}

fn generate_luarc_json(lsp_dir: &Path) -> Result<String> {
    let lsp_dir_str = lsp_dir.to_string_lossy().replace('\\', "/");

    let json = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/sumneko/lua-language-server/master/setting/schema.json",
        "runtime": {
            "version": "Luau"
        },
        "workspace": {
            "library": [
                lsp_dir_str
            ],
            "checkThirdParty": false
        },
        "diagnostics": {
            "disable": [
                "lowercase-global"
            ],
            "globals": [
                "SYSTEM", "ZOI", "PKG", "BUILD_DIR", "STAGING_DIR", "BUILD_TYPE", "SUBPKG",
                "metadata", "dependencies", "updates", "hooks", "service", "prepare", "package",
                "verify", "test", "uninstall", "cmd", "zcp", "zlicense", "zdoc", "zsed", "zpatch", "zln", "zchmod", "zchown", "zmkdir",
                "zrm", "IMPORT", "INCLUDE", "verifyHash", "verifySignature", "addPgpKey", "UTILS"
            ]
        }
    });

    Ok(serde_json::to_string_pretty(&json)?)
}
