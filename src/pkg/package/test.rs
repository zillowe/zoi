use crate::{cmd, pkg, utils};
use anyhow::{Result, anyhow};
use colored::*;
use mlua::{Lua, LuaSerdeExt};

pub fn run(args: &cmd::package::build::BuildCommand) -> Result<()> {
    println!("Testing package from: {}", args.package_file.display());

    let platform = if let Some(p) = args.platform.first() {
        p.clone()
    } else {
        utils::get_platform()?
    };

    let pkg_for_meta = pkg::lua::parser::parse_lua_package_for_platform(
        args.package_file.to_str().unwrap(),
        &platform,
        None,
        false,
    )?;

    let version = pkg::resolve::get_default_version(&pkg_for_meta, None)?;

    let build_dir = tempfile::Builder::new()
        .prefix(&format!("zoi-test-{}-{}", pkg_for_meta.name, platform))
        .tempdir()?;
    println!("Using build directory: {}", build_dir.path().display());
    let staging_dir = build_dir.path().join("staging");
    std::fs::create_dir_all(&staging_dir)?;

    let subs_to_test = if let Some(subs) = &args.sub {
        subs.clone()
    } else if let Some(subs) = &pkg_for_meta.sub_packages {
        subs.clone()
    } else {
        vec!["".to_string()]
    };

    for sub_package in subs_to_test {
        let sub_pkg_name = if sub_package.is_empty() {
            None
        } else {
            Some(sub_package.as_str())
        };

        if !sub_package.is_empty() {
            println!("--- Testing sub-package: {} ---", sub_package.cyan());
        }

        let lua = Lua::new();
        pkg::lua::functions::setup_lua_environment(
            &lua,
            &platform,
            Some(&version),
            args.package_file.to_str(),
            None,
            sub_pkg_name,
            false,
        )
        .map_err(|e| anyhow!(e.to_string()))?;

        let pkg_table = lua
            .to_value(&pkg_for_meta)
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("PKG", pkg_table)
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("BUILD_DIR", build_dir.path().to_str().unwrap())
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("STAGING_DIR", staging_dir.to_str().unwrap())
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("BUILD_TYPE", args.r#type.as_str())
            .map_err(|e| anyhow!(e.to_string()))?;

        let lua_code = std::fs::read_to_string(&args.package_file)?;
        lua.load(&lua_code)
            .exec()
            .map_err(|e| anyhow!(e.to_string()))?;

        let lua_args = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
        if !sub_package.is_empty() {
            lua_args
                .set("sub", sub_package.clone())
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        if let Ok(prepare_fn) = lua.globals().get::<mlua::Function>("prepare") {
            println!("Running prepare()...");
            prepare_fn
                .call::<()>(lua_args.clone())
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        if let Ok(package_fn) = lua.globals().get::<mlua::Function>("package") {
            println!("Running package()...");
            package_fn
                .call::<()>(lua_args.clone())
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        if let Ok(test_fn) = lua.globals().get::<mlua::Function>("test") {
            println!("Running test()...");
            let success: bool = test_fn
                .call::<bool>(lua_args.clone())
                .map_err(|e| anyhow!(e.to_string()))?;
            if !success {
                return Err(anyhow!(
                    "Package tests failed for sub-package '{}'.",
                    sub_package
                ));
            }
        } else if !sub_package.is_empty() {
            println!(
                "No test() function found for sub-package '{}', skipping.",
                sub_package
            );
        }
    }

    println!("{}", "All tests passed successfully.".green());
    Ok(())
}
