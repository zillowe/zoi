use crate::{pkg, utils};
use anyhow::{Result, anyhow};
use chrono::Utc;
use colored::*;
use mlua::{Lua, LuaSerdeExt, Table};
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
    sign_key: &Option<String>,
    output_dir: Option<&Path>,
) -> Result<()> {
    let pkg_for_meta = pkg::lua::parser::parse_lua_package_for_platform(
        package_file.to_str().unwrap(),
        platform,
        None,
    )?;

    if !pkg_for_meta.types.iter().any(|t| t == build_type) {
        return Err(anyhow!(
            "Build type '{}' not supported by this package. Supported types: {:?}",
            build_type,
            pkg_for_meta.types
        ));
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
        None,
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
        .set("BUILD_TYPE", build_type)
        .map_err(|e| anyhow!(e.to_string()))?;

    let lua_code = fs::read_to_string(package_file)?;
    lua.load(&lua_code)
        .exec()
        .map_err(|e| anyhow!(e.to_string()))?;

    if let Ok(prepare_fn) = lua.globals().get::<mlua::Function>("prepare") {
        println!("Running prepare()...");
        prepare_fn
            .call::<()>(())
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    if let Ok(package_fn) = lua.globals().get::<mlua::Function>("package") {
        println!("Running package()...");
        package_fn
            .call::<()>(())
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    if let Ok(build_ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in build_ops.sequence_values::<Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;
            if let Ok(op_type) = op.get::<String>("op")
                && op_type == "zcp"
            {
                let source: String = op.get("source").map_err(|e| anyhow!(e.to_string()))?;
                let mut destination: String =
                    op.get("destination").map_err(|e| anyhow!(e.to_string()))?;

                let source_path = build_dir.path().join(&source);

                destination = destination.replace("${pkgstore}", "data/pkgstore");
                destination = destination.replace("${createpkgdir}", "data/createpkgdir");
                destination = destination.replace("${usrroot}", "data/usrroot");
                destination = destination.replace("${usrhome}", "data/usrhome");

                let dest_path = if destination.starts_with("${createpkgdir}") {
                    let create_pkg_dir_str: String = lua
                        .globals()
                        .get("CREATE_PKG_DIR")
                        .map_err(|e| anyhow!(e.to_string()))?;
                    let remaining_path = destination.strip_prefix("${createpkgdir}").unwrap();
                    Path::new(&create_pkg_dir_str).join(remaining_path.trim_start_matches('/'))
                } else {
                    staging_dir.join(&destination)
                };

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
        let verification_passed: bool = verify_fn
            .call::<bool>(())
            .map_err(|e| anyhow!(e.to_string()))?;
        if !verification_passed {
            if !utils::ask_for_confirmation(
                "Package verification failed. This package may be unsafe. Continue?",
                false,
            ) {
                return Err(anyhow!(
                    "Build aborted by user due to verification failure."
                ));
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
    let output_base = if let Some(dir) = output_dir {
        dir.to_path_buf()
    } else {
        package_file.parent().unwrap().to_path_buf()
    };
    let output_path = output_base.join(output_filename);

    let file = File::create(&output_path)?;
    let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
    let mut tar_builder = TarBuilder::new(encoder);
    tar_builder.append_dir_all(".", &staging_dir)?;
    tar_builder.finish()?;

    println!(
        "{}",
        format!("Successfully built package: {}", output_path.display()).green()
    );

    if let Some(key_id) = sign_key {
        println!("Signing package with key '{}'...", key_id.cyan());
        let signature_path = output_path.with_extension("pkg.tar.zst.sig");
        if signature_path.exists() {
            fs::remove_file(&signature_path)?;
        }
        pkg::pgp::sign_detached(&output_path, &signature_path, key_id)?;
        println!(
            "{}",
            format!(
                "Successfully created signature: {}",
                signature_path.display()
            )
            .green()
        );
    }

    Ok(())
}

pub fn run(
    package_file: &Path,
    build_type: &str,
    platforms: &[String],
    sign_key: Option<String>,
    output_dir: Option<&Path>,
) -> Result<()> {
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
        return Err(anyhow!(
            "Building for 'all' platforms is not supported in this flow yet. Please specify platforms explicitly."
        ));
    }

    for platform in &platforms_to_build {
        println!("--- Building for platform: {} ---", platform.cyan());
        if let Err(e) =
            build_for_platform(package_file, build_type, platform, &sign_key, output_dir)
        {
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
