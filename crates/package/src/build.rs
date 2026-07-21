/// Orchestrates the Zoi package build process.
///
/// This module is responsible for turning a `.pkg.lua` definition into a
/// distributable `.zpa` archive. It:
/// - Executes the `prepare()`, `build()`, and `package()` Lua functions.
/// - Manages the staging area where files are organized into Zoi's data structure.
/// - Generates accompanying metadata: `.hash` (SHA-512), `.size`, and `.files`.
/// - Supports native builds, Docker-based builds, and cross-compilation via CI tags.
/// - Handles optional PGP signing of the resulting archive.
use anyhow::{Result, anyhow};
use colored::*;
use mlua::{Lua, LuaSerdeExt, Table};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder as TarBuilder};
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::types::{
    self, PoolFileEntry, PooledZpaManifest, Scope, ScopeMapping, SubPackageMapping,
};
use zoi_core::utils;
use zoi_lua;
use zoi_resolver::resolve;
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;

pub fn resolve_build_type(
    requested: Option<&str>,
    supported: &[String],
    pkg_name: &str,
) -> Result<String> {
    if let Some(t) = requested {
        if !supported.iter().any(|s| s == t) {
            return Err(anyhow!(
                "Build type '{}' not supported by package '{}'. Supported types: {:?}",
                t,
                pkg_name,
                supported
            ));
        }
        return Ok(t.to_string());
    }

    if supported.iter().any(|t| t == "pre-compiled") {
        Ok("pre-compiled".to_string())
    } else if supported.iter().any(|t| t == "source") {
        Ok("source".to_string())
    } else if let Some(first) = supported.first() {
        Ok(first.clone())
    } else {
        Err(anyhow!(
            "No build types supported by package '{}'.",
            pkg_name
        ))
    }
}

pub fn get_build_dependencies(
    package_file: &Path,
    build_type: Option<&str>,
    platform: &str,
    version_override: Option<&str>,
    quiet: bool,
) -> Result<Option<Vec<String>>> {
    let pkg_for_meta = zoi_lua::parser::parse_lua_package_for_platform(
        package_file
            .to_str()
            .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", package_file))?,
        platform,
        version_override,
        None,
        quiet,
    )?;

    let resolved_build_type =
        resolve_build_type(build_type, &pkg_for_meta.types, &pkg_for_meta.name)?;

    if let Some(deps) = &pkg_for_meta.dependencies
        && let Some(build_deps) = &deps.build
    {
        let group = match build_deps {
            types::BuildDependencies::Group(g) => Some(g),
            types::BuildDependencies::Typed(t) => t.types.get(&resolved_build_type),
        };

        if let Some(g) = group {
            let mut all_deps = Vec::new();
            collect_deps_from_group_no_prompt(g, &mut all_deps);
            return Ok(Some(all_deps));
        }
    }

    Ok(None)
}

fn collect_deps_from_group_no_prompt(group: &types::DependencyGroup, deps: &mut Vec<String>) {
    match group {
        types::DependencyGroup::Simple(d) => {
            deps.extend(d.clone());
        }
        types::DependencyGroup::Complex(g) => {
            deps.extend(g.required.clone());
            deps.extend(g.optional.clone());
            for option_group in &g.options {
                if option_group.all {
                    deps.extend(option_group.depends.clone());
                } else if let Some(dep) = option_group.depends.first() {
                    deps.push(dep.clone());
                }
            }
            if let Some(sub_deps_map) = &g.sub_packages {
                for sub_group in sub_deps_map.values() {
                    collect_deps_from_group_no_prompt(sub_group, deps);
                }
            }
        }
    }
}

fn process_build_operations(
    lua: &Lua,
    _sub_package: &str,
    pkg_lua_dir_str: &str,
    build_dir_path: &Path,
    target_staging_dir: &Path,
    quiet: bool,
) -> Result<()> {
    if let Ok(build_ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in build_ops.sequence_values::<Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;
            let op_type: String = op.get("op").map_err(|e| anyhow!(e.to_string()))?;

            let resolve_dest = |dest: String| -> String {
                dest.replace("${pkgstore}", "pkgstore")
                    .replace("${createpkgdir}", "createpkgdir")
                    .replace("${usrroot}", "usrroot")
                    .replace("${usrhome}", "usrhome")
            };

            match op_type.as_str() {
                "zcp" => {
                    let source: String = op.get("source").map_err(|e| anyhow!(e.to_string()))?;
                    let destination: String =
                        op.get("destination").map_err(|e| anyhow!(e.to_string()))?;

                    let source_path = if source.contains("${pkgluadir}") {
                        Path::new(&source.replace("${pkgluadir}", pkg_lua_dir_str)).to_path_buf()
                    } else {
                        build_dir_path.join(&source)
                    };

                    let dest_rel = resolve_dest(destination);

                    if !utils::is_safe_path(target_staging_dir, Path::new(&dest_rel)) {
                        return Err(anyhow!(
                            "Path traversal detected in zcp destination: {}",
                            dest_rel
                        ));
                    }

                    let dest_path = target_staging_dir.join(&dest_rel);

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
                        println!("Staged '{}' to '{}'", source, dest_rel);
                    }
                }
                "zln" => {
                    let mut target: String =
                        op.get("target").map_err(|e| anyhow!(e.to_string()))?;
                    let link: String = op.get("link").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(link);

                    target = target.replace("${pkgstore}", "pkgstore");
                    target = target.replace("${createpkgdir}", "createpkgdir");
                    target = target.replace("${usrroot}", "usrroot");
                    target = target.replace("${usrhome}", "usrhome");

                    if !utils::is_safe_path(target_staging_dir, Path::new(&dest_rel)) {
                        return Err(anyhow!("Path traversal detected in zln link: {}", dest_rel));
                    }

                    let link_path = target_staging_dir.join(&dest_rel);
                    if let Some(parent) = link_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    utils::symlink_file(Path::new(&target), &link_path)?;
                    if !quiet {
                        println!("Created symlink '{}' -> '{}'", dest_rel, target);
                    }
                }
                "zchmod" => {
                    let path: String = op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                    let mode: u32 = op.get("mode").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(target_staging_dir, Path::new(&dest_rel)) {
                        return Err(anyhow!(
                            "Path traversal detected in zchmod path: {}",
                            dest_rel
                        ));
                    }

                    let full_path = target_staging_dir.join(&dest_rel);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(full_path, fs::Permissions::from_mode(mode))?;
                    }
                    if !quiet {
                        println!("Set permissions {} on '{}'", mode, dest_rel);
                    }
                }
                "zchown" => {
                    let path: String = op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                    let owner: String = op.get("owner").map_err(|e| anyhow!(e.to_string()))?;
                    let group: String = op.get("group").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(target_staging_dir, Path::new(&dest_rel)) {
                        return Err(anyhow!(
                            "Path traversal detected in zchown path: {}",
                            dest_rel
                        ));
                    }

                    #[cfg(unix)]
                    let full_path = target_staging_dir.join(&dest_rel);
                    #[cfg(unix)]
                    utils::set_path_owner(&full_path, &owner, &group)?;
                    if !quiet {
                        println!("Set ownership {}:{} on '{}'", owner, group, dest_rel);
                    }
                }
                "zmkdir" => {
                    let path: String = op.get("path").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(target_staging_dir, Path::new(&dest_rel)) {
                        return Err(anyhow!(
                            "Path traversal detected in zmkdir path: {}",
                            dest_rel
                        ));
                    }

                    let full_path = target_staging_dir.join(&dest_rel);
                    fs::create_dir_all(full_path)?;
                    if !quiet {
                        println!("Created directory '{}'", dest_rel);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn build_for_platform(
    package_file: &Path,
    build_type: Option<&str>,
    platform: &str,
    sign_key: &Option<String>,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<&Vec<String>>,
    quiet: bool,
    fakeroot: bool,
) -> Result<()> {
    let pkg_lua_dir_str = package_file
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| anyhow!("Could not get parent directory of package file"))?;
    let pkg_for_meta = zoi_lua::parser::parse_lua_package_for_platform(
        package_file
            .to_str()
            .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", package_file))?,
        platform,
        version_override,
        None,
        quiet,
    )?;

    if let Some(allowed_platforms) = &pkg_for_meta.platforms
        && !utils::is_platform_compatible(platform, allowed_platforms)
    {
        if !quiet {
            println!(
                "{} Skipping build for platform {}: package only supports {:?}",
                "::".bold().yellow(),
                platform.cyan(),
                allowed_platforms
            );
        }
        return Ok(());
    }

    let resolved_build_type =
        resolve_build_type(build_type, &pkg_for_meta.types, &pkg_for_meta.name)?;

    let version = if let Some(v) = version_override {
        v.to_string()
    } else {
        resolve::get_default_version(&pkg_for_meta, None)?
    };

    let build_dir = Builder::new()
        .prefix(&format!("zoi-build-{}-{}", pkg_for_meta.name, platform))
        .tempdir()?;
    if !quiet {
        println!("Using build directory: {}", build_dir.path().display());
    }

    let mut skip_prepare = false;
    if let Some(parent) = package_file.parent()
        && parent.join(".zoi-prepared").exists()
    {
        if !quiet {
            println!(
                "{} Detected pre-prepared source bundle, copying files...",
                "::".bold().blue()
            );
        }
        utils::copy_dir_all(parent, build_dir.path())?;
        skip_prepare = true;
    }

    let staging_dir = build_dir.path().join("staging");
    fs::create_dir_all(&staging_dir)?;

    let pool_dir = staging_dir.join("pool");
    fs::create_dir_all(&pool_dir)?;

    let mut pool: BTreeMap<String, PoolFileEntry> = BTreeMap::new();
    let mut mappings: BTreeMap<String, SubPackageMapping> = BTreeMap::new();

    let subs_to_build = if let Some(subs) = sub_packages {
        subs.clone()
    } else if let Some(subs) = &pkg_for_meta.sub_packages {
        subs.clone()
    } else {
        vec!["".to_string()]
    };

    let scopes_to_process =
        pkg_for_meta
            .scopes
            .clone()
            .unwrap_or(vec![Scope::User, Scope::System, Scope::Project]);

    // Initial prepare and build (Shared across all scopes)
    let lua_shared = Lua::new();
    zoi_lua::functions::setup_lua_environment(
        &lua_shared,
        platform,
        Some(&version),
        package_file.to_str(),
        None,
        Some(build_dir.path().to_str().unwrap_or("")),
        None, // No staging dir for shared build
        None,
        Some(pkg_for_meta.scope),
        quiet,
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    let lua_code = fs::read_to_string(package_file)?;
    lua_shared
        .load(&lua_code)
        .exec()
        .map_err(|e| anyhow!(e.to_string()))?;

    let args_shared = lua_shared
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    if !skip_prepare && let Ok(prepare_fn) = lua_shared.globals().get::<mlua::Function>("prepare") {
        if !quiet {
            println!("Running prepare()...");
        }
        prepare_fn
            .call::<()>(args_shared.clone())
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    if let Ok(build_fn) = lua_shared.globals().get::<mlua::Function>("build") {
        if !quiet {
            println!("Running build()...");
        }
        build_fn
            .call::<()>(args_shared)
            .map_err(|e| anyhow!(e.to_string()))?;
    }

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

        let mut sub_mapping = SubPackageMapping {
            scopes: BTreeMap::new(),
        };

        for scope in &scopes_to_process {
            if !quiet {
                println!("  {} Staging for scope: {:?}", "::".bold().blue(), scope);
            }

            let lua = Lua::new();
            let v_staging = Builder::new().prefix("zoi-vstage-").tempdir()?;

            zoi_lua::functions::setup_lua_environment(
                &lua,
                platform,
                Some(&version),
                package_file.to_str(),
                None,
                Some(build_dir.path().to_str().unwrap_or("")),
                Some(v_staging.path().to_str().unwrap_or("")),
                sub_pkg_name,
                Some(*scope),
                true, // Always quiet for scope loops
            )
            .map_err(|e| anyhow!(e.to_string()))?;

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
                        .ok_or_else(|| anyhow!("build_dir path contains invalid UTF-8"))?,
                )
                .map_err(|e| anyhow!(e.to_string()))?;
            lua.globals()
                .set(
                    "STAGING_DIR",
                    v_staging
                        .path()
                        .to_str()
                        .ok_or_else(|| anyhow!("v_staging path contains invalid UTF-8"))?,
                )
                .map_err(|e| anyhow!(e.to_string()))?;
            lua.globals()
                .set("BUILD_TYPE", resolved_build_type.as_str())
                .map_err(|e| anyhow!(e.to_string()))?;

            lua.load(&lua_code)
                .exec()
                .map_err(|e| anyhow!(e.to_string()))?;

            let args = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
            if !sub_package.is_empty() {
                args.set("sub", sub_package.clone())
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            if let Ok(package_fn) = lua.globals().get::<mlua::Function>("package") {
                package_fn
                    .call::<()>(args.clone())
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            process_build_operations(
                &lua,
                &sub_package,
                pkg_lua_dir_str,
                build_dir.path(),
                v_staging.path(),
                true,
            )?;

            let mut scope_mapping = ScopeMapping::default();
            super::pool::pool_files(v_staging.path(), &pool_dir, &mut pool, &mut scope_mapping)?;

            sub_mapping.scopes.insert(*scope, scope_mapping);

            if *scope == pkg_for_meta.scope {
                // Run verify and test only for default scope to ensure sanity
                if let Ok(verify_fn) = lua.globals().get::<mlua::Function>("verify") {
                    let verification_passed: bool =
                        match verify_fn.call::<mlua::Value>(args.clone()) {
                            Ok(mlua::Value::Boolean(b)) => b,
                            Ok(_) => true, // Legacy behavior
                            Err(e) => return Err(anyhow!("Verification failed: {}", e)),
                        };
                    if !verification_passed {
                        return Err(anyhow!("Package verification failed."));
                    }
                }
            }
        }
        mappings.insert(sub_package, sub_mapping);
    }

    if platform.starts_with("linux")
        && let Err(e) = super::relocate::relocate_elfs(&pool_dir, quiet)
    {
        eprintln!(
            "{} Failed to relocate ELF binaries in pool: {}",
            "Warning:".yellow(),
            e
        );
    }

    let pooled_manifest = PooledZpaManifest {
        version: "2".to_string(),
        pool,
        mappings,
    };

    let manifest_json = serde_json::to_string_pretty(&pooled_manifest)?;
    fs::write(staging_dir.join("manifest.json"), manifest_json)?;

    fs::copy(
        package_file,
        staging_dir.join(
            package_file
                .file_name()
                .ok_or_else(|| anyhow!("package_file should have a name"))?,
        ),
    )?;

    let output_filename = format!("{}-{}-{}.zpa", pkg_for_meta.name, version, platform);
    let output_base = if let Some(dir) = output_dir {
        dir.to_path_buf()
    } else {
        package_file
            .parent()
            .ok_or_else(|| anyhow!("package_file should have a parent directory"))?
            .to_path_buf()
    };
    let output_path = output_base.join(output_filename);

    {
        let file = File::create(&output_path)?;
        let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
        let mut tar_builder = TarBuilder::new(encoder);

        if fakeroot {
            if !quiet {
                println!(
                    "{} Applying fakeroot (UID/GID 0) to archive...",
                    "::".bold().blue()
                );
            }
            for entry in WalkDir::new(&staging_dir).min_depth(1) {
                let entry = entry?;
                let path = entry.path();
                let rel_path = path.strip_prefix(&staging_dir)?;

                let mut header = tar::Header::new_gnu();
                let metadata = fs::symlink_metadata(path)?;

                header.set_metadata(&metadata);
                header.set_uid(0);
                header.set_gid(0);
                header.set_username("root")?;
                header.set_groupname("root")?;

                if metadata.is_dir() {
                    tar_builder.append_data(&mut header, rel_path, std::io::empty())?;
                } else if metadata.is_symlink() {
                    let target = fs::read_link(path)?;
                    tar_builder.append_link(&mut header, rel_path, target)?;
                } else {
                    let mut file = File::open(path)?;
                    tar_builder.append_data(&mut header, rel_path, &mut file)?;
                }
            }
        } else {
            tar_builder.append_dir_all(".", &staging_dir)?;
        }
        tar_builder.finish()?;
    }

    // Legacy metadata files for compatibility
    let mut files_list = Vec::new();
    for p in pooled_manifest.pool.keys() {
        files_list.push(format!("pool/{}", p));
    }
    files_list.push("manifest.json".to_string());
    files_list.push(
        package_file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    files_list.sort();

    let files_manifest_path = PathBuf::from(format!("{}.files", output_path.display()));
    fs::write(&files_manifest_path, files_list.join("\n"))?;

    let hash_path = PathBuf::from(format!("{}.hash", output_path.display()));
    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| anyhow!("Output path contains invalid UTF-8: {:?}", output_path))?;
    let hash = zoi_core::hash::calculate_file_hash(
        Path::new(output_path_str),
        zoi_core::hash::HashAlgorithm::Sha512,
    )?;
    fs::write(
        &hash_path,
        format!(
            "{}  {}\n",
            hash,
            output_path
                .file_name()
                .ok_or_else(|| anyhow!("output_path should have a name"))?
                .to_str()
                .ok_or_else(|| anyhow!("output_filename should be valid UTF-8"))?
        ),
    )?;

    let size_path = PathBuf::from(format!("{}.size", output_path.display()));
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
        let signature_path = PathBuf::from(format!("{}.sig", output_path.display()));
        if signature_path.exists() {
            fs::remove_file(&signature_path)?;
        }
        zoi_core::pgp::sign_detached(&output_path, &signature_path, key_id)?;
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
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    quiet: bool,
    method: &str,
    image: Option<&str>,
    fakeroot: bool,
    install_deps: bool,
) -> Result<()> {
    let mut _temp_zsa_dir = None;
    let mut actual_package_file = package_file.to_path_buf();
    let mut default_output_dir = None;

    if package_file.to_string_lossy().ends_with(".zsa") {
        if !quiet {
            println!(
                "{} Extracting source bundle: {}",
                "::".bold().blue(),
                package_file.display()
            );
        }

        if output_dir.is_none() {
            default_output_dir = package_file.parent().map(|p| p.to_path_buf());
        }

        let temp_dir = Builder::new().prefix("zoi-zsa-extract-").tempdir()?;
        let file = File::open(package_file)?;
        let decoder = ZstdDecoder::new(file)?;
        let mut archive = Archive::new(decoder);
        archive.unpack(temp_dir.path())?;

        // Locate the .pkg.lua file inside the bundle
        let mut pkg_lua = None;
        for entry in WalkDir::new(temp_dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name().to_string_lossy().ends_with(".pkg.lua") {
                pkg_lua = Some(entry.path().to_path_buf());
                break;
            }
        }

        actual_package_file = pkg_lua
            .ok_or_else(|| anyhow!("Could not find .pkg.lua file inside the .zsa bundle."))?;
        _temp_zsa_dir = Some(temp_dir);
    }

    let package_file = actual_package_file.as_path();
    let output_dir = output_dir.or(default_output_dir.as_deref());

    if method == "docker" {
        let docker_image = image.ok_or_else(|| {
            anyhow!("An image must be specified when using the 'docker' build method.")
        })?;
        return super::docker::run(
            package_file,
            build_type,
            platforms,
            sign_key,
            output_dir,
            version_override,
            sub_packages,
            docker_image,
            fakeroot,
            install_deps,
        );
    }

    if method == "bwrap" {
        return super::bwrap::run(
            package_file,
            build_type,
            platforms,
            sign_key,
            output_dir,
            version_override,
            sub_packages,
            fakeroot,
            install_deps,
        );
    }

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

    let mut any_failed = false;

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
            fakeroot,
        ) {
            eprintln!(
                "{}: Failed to build for platform {}: {}",
                "Error".red().bold(),
                platform.red(),
                e
            );
            any_failed = true;
        }
    }

    if any_failed {
        return Err(anyhow!("One or more platform builds failed"));
    }

    Ok(())
}
