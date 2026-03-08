use crate::{pkg, utils};
use anyhow::{Result, anyhow};
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
    version_override: Option<&str>,
    sub_packages: Option<&Vec<String>>,
    quiet: bool,
) -> Result<()> {
    let pkg_lua_dir_str = package_file
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| anyhow!("Could not get parent directory of package file"))?;
    let pkg_for_meta = pkg::lua::parser::parse_lua_package_for_platform(
        package_file
            .to_str()
            .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", package_file))?,
        platform,
        version_override,
        quiet,
    )?;

    if !pkg_for_meta.types.iter().any(|t| t == build_type) {
        return Err(anyhow!(
            "Build type '{}' not supported by this package. Supported types: {:?}",
            build_type,
            pkg_for_meta.types
        ));
    }

    let version = if let Some(v) = version_override {
        v.to_string()
    } else {
        pkg::resolve::get_default_version(&pkg_for_meta, None)?
    };

    let build_dir = Builder::new()
        .prefix(&format!("zoi-build-{}-{}", pkg_for_meta.name, platform))
        .tempdir()?;
    if !quiet {
        println!("Using build directory: {}", build_dir.path().display());
    }
    let staging_dir = build_dir.path().join("staging");
    fs::create_dir_all(&staging_dir)?;

    let subs_to_build = if let Some(subs) = sub_packages {
        subs.clone()
    } else if let Some(subs) = &pkg_for_meta.sub_packages {
        subs.clone()
    } else {
        vec!["".to_string()]
    };

    for sub_package in subs_to_build {
        let sub_pkg_name = if sub_package.is_empty() {
            None
        } else {
            Some(sub_package.as_str())
        };

        if !sub_package.is_empty() && !quiet {
            println!(
                "{} Building sub-package: {}",
                "::".bold().blue(),
                sub_package.cyan()
            );
        }

        let lua = Lua::new();
        pkg::lua::functions::setup_lua_environment(
            &lua,
            platform,
            Some(&version),
            package_file.to_str(),
            None,
            sub_pkg_name,
            quiet,
        )
        .map_err(|e| {
            anyhow!(
                "Failed to setup Lua build environment for '{}': {}",
                package_file.display(),
                e
            )
        })?;
        let pkg_table = lua
            .to_value(&pkg_for_meta)
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("PKG", pkg_table)
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set(
                "BUILD_DIR",
                build_dir
                    .path()
                    .to_str()
                    .expect("build_dir should be valid UTF-8"),
            )
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set(
                "STAGING_DIR",
                staging_dir
                    .to_str()
                    .expect("staging_dir should be valid UTF-8"),
            )
            .map_err(|e| anyhow!(e.to_string()))?;
        lua.globals()
            .set("BUILD_TYPE", build_type)
            .map_err(|e| anyhow!(e.to_string()))?;

        let lua_code = fs::read_to_string(package_file)?;
        lua.load(&lua_code).exec().map_err(|e| {
            anyhow!(
                "Failed to execute Lua package file '{}' during build:\n{}",
                package_file.display(),
                e
            )
        })?;

        let args = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
        if !sub_package.is_empty() {
            args.set("sub", sub_package.clone())
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        if let Ok(prepare_fn) = lua.globals().get::<mlua::Function>("prepare") {
            if !quiet {
                println!("Running prepare()...");
            }
            prepare_fn.call::<()>(args.clone()).map_err(|e| {
                anyhow!(
                    "The 'prepare' function in '{}' failed for sub-package '{}':\n{}",
                    package_file.display(),
                    sub_package,
                    e
                )
            })?;
        }

        if let Ok(package_fn) = lua.globals().get::<mlua::Function>("package") {
            if !quiet {
                println!("Running package()...");
            }
            package_fn.call::<()>(args.clone()).map_err(|e| {
                anyhow!(
                    "The 'package' function in '{}' failed for sub-package '{}':\n{}",
                    package_file.display(),
                    sub_package,
                    e
                )
            })?;
        }

        if let Ok(build_ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
            for op in build_ops.sequence_values::<Table>() {
                let op = op.map_err(|e| anyhow!(e.to_string()))?;
                let op_type: String = op.get("op").map_err(|e| anyhow!(e.to_string()))?;

                let data_prefix = if sub_package.is_empty() {
                    "data".to_string()
                } else {
                    format!("data/{}", sub_package)
                };

                match op_type.as_str() {
                    "zcp" => {
                        let source: String =
                            op.get("source").map_err(|e| anyhow!(e.to_string()))?;
                        let mut destination: String =
                            op.get("destination").map_err(|e| anyhow!(e.to_string()))?;

                        let source_path = if source.contains("${pkgluadir}") {
                            Path::new(&source.replace("${pkgluadir}", pkg_lua_dir_str))
                                .to_path_buf()
                        } else {
                            build_dir.path().join(&source)
                        };

                        destination = destination
                            .replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        destination = destination
                            .replace("${createpkgdir}", &format!("{}/createpkgdir", data_prefix));
                        destination =
                            destination.replace("${usrroot}", &format!("{}/usrroot", data_prefix));
                        destination =
                            destination.replace("${usrhome}", &format!("{}/usrhome", data_prefix));

                        let dest_path = staging_dir.join(&destination);

                        if let Some(parent) = dest_path.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        if source_path.is_dir() {
                            for entry in WalkDir::new(&source_path)
                                .into_iter()
                                .filter_map(|e| e.ok())
                            {
                                let target_path =
                                    dest_path.join(entry.path().strip_prefix(&source_path)?);
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
                        if !quiet {
                            println!("Staged '{}' to '{}'", source, destination);
                        }
                    }
                    "zln" => {
                        let mut target: String =
                            op.get("target").map_err(|e| anyhow!(e.to_string()))?;
                        let mut link: String =
                            op.get("link").map_err(|e| anyhow!(e.to_string()))?;

                        target =
                            target.replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        link = link.replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        link = link
                            .replace("${createpkgdir}", &format!("{}/createpkgdir", data_prefix));
                        link = link.replace("${usrroot}", &format!("{}/usrroot", data_prefix));
                        link = link.replace("${usrhome}", &format!("{}/usrhome", data_prefix));

                        let link_path = staging_dir.join(&link);
                        if let Some(parent) = link_path.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        utils::symlink_file(Path::new(&target), &link_path)?;
                        if !quiet {
                            println!("Created symlink '{}' -> '{}'", link, target);
                        }
                    }
                    "zchmod" => {
                        let mut path: String =
                            op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                        let mode: u32 = op.get("mode").map_err(|e| anyhow!(e.to_string()))?;

                        path = path.replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        path = path
                            .replace("${createpkgdir}", &format!("{}/createpkgdir", data_prefix));
                        path = path.replace("${usrroot}", &format!("{}/usrroot", data_prefix));
                        path = path.replace("${usrhome}", &format!("{}/usrhome", data_prefix));

                        let _full_path = staging_dir.join(&path);
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            fs::set_permissions(_full_path, fs::Permissions::from_mode(mode))?;
                        }
                        if !quiet {
                            println!("Set permissions {} on '{}'", mode, path);
                        }
                    }
                    "zchown" => {
                        let mut path: String =
                            op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                        let owner: String = op.get("owner").map_err(|e| anyhow!(e.to_string()))?;
                        let group: String = op.get("group").map_err(|e| anyhow!(e.to_string()))?;

                        path = path.replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        path = path
                            .replace("${createpkgdir}", &format!("{}/createpkgdir", data_prefix));
                        path = path.replace("${usrroot}", &format!("{}/usrroot", data_prefix));
                        path = path.replace("${usrhome}", &format!("{}/usrhome", data_prefix));

                        let full_path = staging_dir.join(&path);
                        utils::set_path_owner(&full_path, &owner, &group)?;
                        if !quiet {
                            println!("Set ownership {}:{} on '{}'", owner, group, path);
                        }
                    }
                    "zmkdir" => {
                        let mut path: String =
                            op.get("path").map_err(|e| anyhow!(e.to_string()))?;

                        path = path.replace("${pkgstore}", &format!("{}/pkgstore", data_prefix));
                        path = path
                            .replace("${createpkgdir}", &format!("{}/createpkgdir", data_prefix));
                        path = path.replace("${usrroot}", &format!("{}/usrroot", data_prefix));
                        path = path.replace("${usrhome}", &format!("{}/usrhome", data_prefix));

                        let full_path = staging_dir.join(&path);
                        fs::create_dir_all(full_path)?;
                        if !quiet {
                            println!("Created directory '{}'", path);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Ok(verify_fn) = lua.globals().get::<mlua::Function>("verify") {
            if !quiet {
                println!("Running verify()...");
            }
            let verification_passed: bool = verify_fn.call::<bool>(args.clone()).map_err(|e| {
                anyhow!(
                    "The 'verify' function in '{}' failed for sub-package '{}':\n{}",
                    package_file.display(),
                    sub_package,
                    e
                )
            })?;
            if !verification_passed {
                if !utils::ask_for_confirmation(
                    "Package verification failed. This package may be unsafe. Continue?",
                    false,
                ) {
                    return Err(anyhow!(
                        "Build aborted by user due to verification failure."
                    ));
                }
            } else if !quiet {
                println!("Package verification passed.");
            }
        }
    }

    let mut files_list = Vec::new();
    for entry in WalkDir::new(&staging_dir) {
        let entry = entry?;
        if entry.file_type().is_file()
            && let Ok(relative_path) = entry.path().strip_prefix(&staging_dir)
        {
            files_list.push(relative_path.to_string_lossy().replace('\\', "/"));
        }
    }
    files_list.sort();

    let manifest_content = files_list.join("\n  - ").to_string();
    fs::write(staging_dir.join("manifest.yaml"), manifest_content)?;

    fs::copy(
        package_file,
        staging_dir.join(
            package_file
                .file_name()
                .expect("package_file should have a name"),
        ),
    )?;

    let output_filename = format!("{}-{}-{}.pkg.tar.zst", pkg_for_meta.name, version, platform);
    let output_base = if let Some(dir) = output_dir {
        dir.to_path_buf()
    } else {
        package_file
            .parent()
            .expect("package_file should have a parent directory")
            .to_path_buf()
    };
    let output_path = output_base.join(output_filename);

    {
        let file = File::create(&output_path)?;
        let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
        let mut tar_builder = TarBuilder::new(encoder);
        tar_builder.append_dir_all(".", &staging_dir)?;
        tar_builder.finish()?;
    }

    let files_manifest_path = output_path.with_extension("pkg.tar.zst.files");
    fs::write(&files_manifest_path, files_list.join("\n"))?;

    let hash_path = output_path.with_extension("pkg.tar.zst.hash");
    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| anyhow!("Output path contains invalid UTF-8: {:?}", output_path))?;
    let hash = pkg::helper::get_hash(output_path_str, pkg::helper::HashType::Sha512)?;
    fs::write(
        &hash_path,
        format!(
            "{}  {}\n",
            hash,
            output_path
                .file_name()
                .expect("output_path should have a name")
                .to_str()
                .expect("output_filename should be valid UTF-8")
        ),
    )?;

    let size_path = output_path.with_extension("pkg.tar.zst.size");
    let compressed_size = fs::metadata(&output_path)?.len();
    let uncompressed_size: u64 = WalkDir::new(&staging_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    fs::write(
        &size_path,
        format!(
            "down: {}\ninstall: {}\n",
            compressed_size, uncompressed_size
        ),
    )?;

    if !quiet {
        println!(
            "{}",
            format!("Successfully built package: {}", output_path.display()).green()
        );
    }

    if let Some(key_id) = sign_key {
        if !quiet {
            println!("Signing package with key '{}'...", key_id.cyan());
        }
        let signature_path = output_path.with_extension("pkg.tar.zst.sig");
        if signature_path.exists() {
            fs::remove_file(&signature_path)?;
        }
        pkg::pgp::sign_detached(&output_path, &signature_path, key_id)?;
        if !quiet {
            println!(
                "{}",
                format!(
                    "Successfully created signature: {}",
                    signature_path.display()
                )
                .green()
            );
        }
    }

    Ok(())
}

pub fn run(
    package_file: &Path,
    build_type: &str,
    platforms: &[String],
    sign_key: Option<String>,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        println!("Building package from: {}", package_file.display());
    }

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
        if !quiet {
            println!(
                "{} Building for platform: {}",
                "::".bold().blue(),
                platform.cyan()
            );
        }
        if let Err(e) = build_for_platform(
            package_file,
            build_type,
            platform,
            &sign_key,
            output_dir,
            version_override,
            sub_packages.as_ref(),
            quiet,
        ) {
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
