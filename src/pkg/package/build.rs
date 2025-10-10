use crate::{pkg, utils};
use chrono::Utc;
use colored::*;
use mlua::{Lua, LuaSerdeExt, Table};
use std::error::Error;
use std::fs::{self, File};
use std::path::Path;
use tar::Builder as TarBuilder;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::write::Encoder as ZstdEncoder;

fn build_for_platform(
    package_file: &Path,
    build_type: &str,
    platform: &str,
) -> Result<(), Box<dyn Error>> {
    let pkg_for_meta = pkg::lua::parser::parse_lua_package_for_platform(
        package_file.to_str().unwrap(),
        platform,
        None,
    )?;

    if !pkg_for_meta.types.iter().any(|t| t == build_type) {
        return Err(format!(
            "Build type '{}' not supported by this package. Supported types: {:?}",
            build_type, pkg_for_meta.types
        )
        .into());
    }

    let version = pkg::resolve::get_default_version(&pkg_for_meta, None)?;

    let build_dir = Builder::new()
        .prefix(&format!("zoi-build-{}-{}", pkg_for_meta.name, platform))
        .tempdir()?;
    println!("Using build directory: {}", build_dir.path().display());
    let staging_dir = build_dir.path().join("staging");
    fs::create_dir_all(&staging_dir)?;

    let lua = Lua::new();
    pkg::lua::functions::setup_lua_environment(
        &lua,
        platform,
        Some(&version),
        package_file.to_str(),
    )?;
    let pkg_table = lua.to_value(&pkg_for_meta)?;
    lua.globals().set("PKG", pkg_table)?;
    lua.globals()
        .set("BUILD_DIR", build_dir.path().to_str().unwrap())?;
    lua.globals()
        .set("STAGING_DIR", staging_dir.to_str().unwrap())?;
    lua.globals().set("BUILD_TYPE", build_type)?;

    let lua_code = fs::read_to_string(package_file)?;
    lua.load(&lua_code).exec()?;

    if let Ok(prepare_fn) = lua.globals().get::<mlua::Function>("prepare") {
        println!("Running prepare()...");
        prepare_fn.call::<()>(())?;
    }

    if let Ok(package_fn) = lua.globals().get::<mlua::Function>("package") {
        println!("Running package()...");
        package_fn.call::<()>(())?;
    }

    if let Ok(build_ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in build_ops.sequence_values::<Table>() {
            let op = op?;
            if let Ok(op_type) = op.get::<String>("op")
                && op_type == "zcp"
            {
                let source: String = op.get("source")?;
                let mut destination: String = op.get("destination")?;

                let source_path = build_dir.path().join(&source);

                destination = destination.replace("${pkgstore}", "data/pkgstore");
                destination = destination.replace("${usrroot}", "data/usrroot");
                destination = destination.replace("${usrhome}", "data/usrhome");

                let dest_path = staging_dir.join(&destination);

                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                if source_path.is_dir() {
                    for entry in WalkDir::new(&source_path)
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        let target_path = dest_path.join(entry.path().strip_prefix(&source_path)?);
                        if entry.file_type().is_dir() {
                            fs::create_dir_all(&target_path)?;
                        } else {
                            if let Some(p) = target_path.parent() {
                                fs::create_dir_all(p)?;
                            }
                            fs::copy(entry.path(), &target_path)?;
                        }
                    }
                } else {
                    fs::copy(&source_path, &dest_path)?;
                }
                println!("Staged '{}' to '{}'", source, destination);
            }
        }
    }

    if let Ok(verify_fn) = lua.globals().get::<mlua::Function>("verify") {
        println!("Running verify()...");
        let verification_passed: bool = verify_fn.call::<bool>(())?;
        if !verification_passed {
            if !utils::ask_for_confirmation(
                "Package verification failed. This package may be unsafe. Continue?",
                false,
            ) {
                return Err("Build aborted by user due to verification failure.".into());
            }
        } else {
            println!("Package verification passed.");
        }
    }

    let manifest_content = format!(
        "zoi_version: {}\nbuild_date: {}",
        env!("CARGO_PKG_VERSION"),
        Utc::now().to_rfc3339()
    );
    fs::write(staging_dir.join("manifest.yaml"), manifest_content)?;

    fs::copy(
        package_file,
        staging_dir.join(package_file.file_name().unwrap()),
    )?;

    let output_filename = format!("{}-{}-{}.pkg.tar.zst", pkg_for_meta.name, version, platform);
    let output_path = package_file.parent().unwrap().join(output_filename);

    let file = File::create(&output_path)?;
    let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
    let mut tar_builder = TarBuilder::new(encoder);
    tar_builder.append_dir_all(".", &staging_dir)?;
    tar_builder.finish()?;

    println!(
        "{}",
        format!("Successfully built package: {}", output_path.display()).green()
    );

    Ok(())
}

pub fn run(
    package_file: &Path,
    build_type: &str,
    platforms: &[String],
) -> Result<(), Box<dyn Error>> {
    println!("Building package from: {}", package_file.display());

    let platforms_to_build: Vec<String> = if platforms.contains(&"current".to_string()) {
        let mut p = platforms.to_vec();
        p.retain(|x| x != "current");
        p.push(utils::get_platform()?);
        p
    } else {
        platforms.to_vec()
    };

    if platforms.contains(&"all".to_string()) {
        return Err(
            "Building for 'all' platforms is not supported in this flow yet. Please specify platforms explicitly."
                .into(),
        );
    }

    for platform in &platforms_to_build {
        println!("--- Building for platform: {} ---", platform.cyan());
        if let Err(e) = build_for_platform(package_file, build_type, platform) {
            eprintln!(
                "{}: Failed to build for platform {}: {}",
                "Error".red().bold(),
                platform.red(),
                e
            );
        }
    }

    Ok(())
}
