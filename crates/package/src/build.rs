//! Orchestrates the Zoi package build process.
//!
//! This module is responsible for turning a `.pkg.lua` definition into a
//! distributable `.zpa` archive. It:
//! - Executes the `prepare()`, `build()`, and `package()` Lua functions.
//! - Manages the staging area where files are organized into Zoi's data
//!   structure.
//! - Generates accompanying metadata: `.hash` (SHA-512), `.size`, and `.files`.
//! - Supports native builds, Docker-based builds, and cross-compilation via CI
//!   tags.
//! - Handles optional PGP signing of the resulting archive.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use mlua::{Lua, LuaSerdeExt, Table};
use tar::{Archive, Builder as TarBuilder};
use tempfile::Builder;
use walkdir::WalkDir;
use zoi_core::types::{
    self, DEBUG_SUB_PACKAGE, PoolFileEntry, PooledZpaManifest, Scope,
    ScopeMapping, StripMode, SubPackageMapping
};
use zoi_core::utils;
use zoi_lua;
use zoi_resolver::resolve;
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;

/// Top-level key of a pooled `manifest.json` that records the SHA-256 of the
/// `.pkg.lua` bundled next to it, as `sha256-<hex>`.
///
/// The installer parses (and so executes) that `.pkg.lua`, but the embedded
/// `manifest.sig` only covers `manifest.json`. Recording the definition's
/// digest inside the signed manifest extends the signature to it. It is an
/// extra JSON key rather than a `PooledZpaManifest` field so that existing
/// readers, which ignore unknown keys, keep parsing new archives unchanged.
pub const PKG_LUA_HASH_KEY: &str = "pkg_lua_sha256";

/// Serializes a pooled manifest for an archive whose bundled package
/// definition is `package_file`, recording the definition's digest under
/// [`PKG_LUA_HASH_KEY`].
///
/// # Errors
///
/// Returns an error if the manifest cannot be serialized or the package file
/// cannot be hashed.
fn pooled_manifest_json(
    manifest: &PooledZpaManifest,
    package_file: &Path
) -> Result<String> {
    let mut value = serde_json::to_value(manifest)?;
    let hash = zoi_core::hash::calculate_file_hash(
        package_file,
        zoi_core::hash::HashAlgorithm::Sha256
    )?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("pooled manifest did not serialize to a map"))?;
    object.insert(
        PKG_LUA_HASH_KEY.to_string(),
        serde_json::Value::String(format!("sha256-{hash}"))
    );
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Resolves the build type to use based on requested type and supported types.
///
/// # Errors
///
/// Returns an error if the requested build type is not supported by the
/// package.
pub fn resolve_build_type(
    requested: Option<&str>,
    supported: &[String],
    pkg_name: &str
) -> Result<Option<String>> {
    if let Some(t) = requested {
        if !supported.iter().any(|s| s == t) {
            return Err(anyhow!(
                "Build type '{t}' not supported by package '{pkg_name}'. \
                 Supported types: {supported:?}",
            ));
        }
        return Ok(Some(t.to_string()));
    }

    if supported.iter().any(|t| t == "pre-compiled") {
        Ok(Some("pre-compiled".to_string()))
    } else if supported.iter().any(|t| t == "source") {
        Ok(Some("source".to_string()))
    } else if let Some(first) = supported.first() {
        Ok(Some(first.clone()))
    } else {
        Ok(None)
    }
}

/// Retrieves build-time dependencies for a package on a specific platform.
///
/// # Errors
///
/// Returns an error if the package file path contains invalid UTF-8 characters,
/// or if parsing the Lua package definition fails.
pub fn get_build_dependencies(
    package_file: &Path,
    build_type: Option<&str>,
    platform: &str,
    version_override: Option<&str>,
    quiet: bool
) -> Result<Option<Vec<String>>> {
    let pkg_for_meta = zoi_lua::parser::parse_lua_package_for_platform(
        package_file.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {}",
                package_file.display()
            )
        })?,
        platform,
        version_override,
        None,
        quiet
    )?;

    let Some(resolved_build_type) = resolve_build_type(
        build_type,
        &pkg_for_meta.types,
        &pkg_for_meta.name
    )?
    else {
        return Ok(None);
    };

    if let Some(deps) = &pkg_for_meta.dependencies
        && let Some(build_deps) = &deps.build
    {
        match build_deps {
            types::BuildDependencies::Group(g) => {
                let mut all_deps = Vec::new();
                collect_deps_from_group_no_prompt(g, &mut all_deps);
                return Ok(Some(all_deps));
            }
            types::BuildDependencies::Typed(t) => {
                if let Some(g) = t.types.get(&resolved_build_type) {
                    let mut all_deps = Vec::new();
                    collect_deps_from_group_no_prompt(g, &mut all_deps);
                    return Ok(Some(all_deps));
                }
            }
            types::BuildDependencies::List(l) => {
                let mut all_deps = Vec::new();
                for entry in l {
                    if entry.build_type == resolved_build_type {
                        all_deps.extend(entry.packages.iter().cloned());
                    }
                }
                if !all_deps.is_empty() {
                    return Ok(Some(all_deps));
                }
            }
        }
    }

    Ok(None)
}

/// Retrieves test-time dependencies for a package on a specific platform.
///
/// # Errors
///
/// Returns an error if the package file path contains invalid UTF-8 characters,
/// or if parsing the Lua package definition fails.
pub fn get_test_dependencies(
    package_file: &Path,
    platform: &str,
    version_override: Option<&str>,
    quiet: bool
) -> Result<Option<Vec<String>>> {
    let pkg_for_meta = zoi_lua::parser::parse_lua_package_for_platform(
        package_file.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {}",
                package_file.display()
            )
        })?,
        platform,
        version_override,
        None,
        quiet
    )?;

    if let Some(deps) = &pkg_for_meta.dependencies
        && let Some(test_deps) = &deps.test
    {
        let mut all_deps = Vec::new();
        collect_deps_from_group_no_prompt(test_deps, &mut all_deps);
        return Ok(Some(all_deps));
    }

    Ok(None)
}

/// Returns the dependency strings that cannot be satisfied inside a `--pure`
/// sysroot.
///
/// A pure build only sees what Zoi installs into its sysroot, which is limited
/// to `zoi:` packages. Any other manager (`pacman:`, `apt:`, `native:`, ...)
/// would be checked and installed on the host instead, leaving the tool
/// missing from the isolated build. Dependencies without an explicit manager
/// prefix are Zoi packages and are therefore allowed.
///
/// # Errors
///
/// Returns an error if any dependency string cannot be parsed.
pub fn non_zoi_dependencies(deps: &[String]) -> Result<Vec<String>> {
    let mut foreign = Vec::new();
    for dep_str in deps {
        let dep = zoi_deps::parse_dependency_string(dep_str)?;
        if dep.manager != "zoi" && !foreign.contains(dep_str) {
            foreign.push(dep_str.clone());
        }
    }
    Ok(foreign)
}

/// Recursively collects dependencies from a dependency group without prompting
/// the user.
fn collect_deps_from_group_no_prompt(
    group: &types::DependencyGroup,
    deps: &mut Vec<String>
) {
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

/// Processes build operations defined in the Lua environment and stages them
/// into the target directory.
fn process_build_operations(
    lua: &Lua,
    _sub_package: &str,
    pkg_lua_dir_str: &str,
    build_dir_path: &Path,
    target_staging_dir: &Path,
    quiet: bool
) -> Result<()> {
    if let Ok(build_ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in build_ops.sequence_values::<Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;
            let op_type: String =
                op.get("op").map_err(|e| anyhow!(e.to_string()))?;

            let resolve_dest = |dest: String| -> String {
                dest.replace("${pkgstore}", "pkgstore")
                    .replace("${createpkgdir}", "createpkgdir")
                    .replace("${usrroot}", "usrroot")
                    .replace("${usrhome}", "usrhome")
            };

            match op_type.as_str() {
                "zcp" => {
                    let source: String =
                        op.get("source").map_err(|e| anyhow!(e.to_string()))?;
                    let destination: String = op
                        .get("destination")
                        .map_err(|e| anyhow!(e.to_string()))?;

                    let mut source_path = if source.contains("${pkgluadir}") {
                        Path::new(
                            &source.replace("${pkgluadir}", pkg_lua_dir_str)
                        )
                        .to_path_buf()
                    } else {
                        build_dir_path.join(&source)
                    };

                    if !source_path.exists() && !source.contains("${pkgluadir}")
                    {
                        let fallback = Path::new(pkg_lua_dir_str).join(&source);
                        if fallback.exists() {
                            source_path = fallback;
                        }
                    }

                    let dest_rel = resolve_dest(destination);

                    if !utils::is_safe_path(
                        target_staging_dir,
                        Path::new(&dest_rel)
                    ) {
                        return Err(anyhow!(
                            "Path traversal detected in zcp destination: \
                             {dest_rel}"
                        ));
                    }

                    let dest_path = target_staging_dir.join(&dest_rel);

                    let source_metadata = match source_path.symlink_metadata() {
                        Ok(m) => m,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if !quiet {
                                println!(
                                    "{} Skipping missing zcp source: \
                                     '{source}' (not found in build dir or \
                                     package dir)",
                                    "::".bold().yellow(),
                                );
                            }
                            continue;
                        }
                        Err(e) => {
                            return Err(anyhow!(
                                "Failed to get metadata for '{}' (resolved \
                                 from '{source}'): {e}",
                                source_path.display(),
                            ));
                        }
                    };

                    if source_metadata.is_dir() {
                        for entry in WalkDir::new(&source_path)
                            .into_iter()
                            .filter_map(Result::ok)
                        {
                            let rel_entry =
                                entry.path().strip_prefix(&source_path)?;
                            let target_path = dest_path.join(rel_entry);

                            let Ok(metadata) = entry.path().symlink_metadata()
                            else {
                                continue; // Skip entries we can't read
                            };

                            if metadata.is_dir() {
                                fs::create_dir_all(&target_path)?;
                            } else if metadata.is_symlink() {
                                if let Some(p) = target_path.parent() {
                                    fs::create_dir_all(p)?;
                                }
                                if let Ok(link_target) =
                                    fs::read_link(entry.path())
                                {
                                    utils::symlink_file(
                                        &link_target,
                                        &target_path
                                    )?;
                                }
                            } else {
                                if let Some(p) = target_path.parent() {
                                    fs::create_dir_all(p)?;
                                }
                                let _ = fs::copy(entry.path(), &target_path);
                            }
                        }
                    } else if source_metadata.is_symlink() {
                        if let Some(parent) = dest_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        if let Ok(link_target) = fs::read_link(&source_path) {
                            utils::symlink_file(&link_target, &dest_path)?;
                        }
                    } else {
                        if let Some(parent) = dest_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::copy(&source_path, &dest_path)?;
                    }

                    if !quiet {
                        println!("Staged '{source}' to '{dest_rel}'");
                    }
                }
                "zln" => {
                    let mut target: String =
                        op.get("target").map_err(|e| anyhow!(e.to_string()))?;
                    let link: String =
                        op.get("link").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(link);

                    target = target.replace("${pkgstore}", "pkgstore");
                    target = target.replace("${createpkgdir}", "createpkgdir");
                    target = target.replace("${usrroot}", "usrroot");
                    target = target.replace("${usrhome}", "usrhome");

                    if !utils::is_safe_path(
                        target_staging_dir,
                        Path::new(&dest_rel)
                    ) {
                        return Err(anyhow!(
                            "Path traversal detected in zln link: {dest_rel}"
                        ));
                    }

                    let link_path = target_staging_dir.join(&dest_rel);
                    if let Some(parent) = link_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    utils::symlink_file(Path::new(&target), &link_path)?;
                    if !quiet {
                        println!("Created symlink '{dest_rel}' -> '{target}'");
                    }
                }
                "zchmod" => {
                    let path: String =
                        op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                    let mode: u32 =
                        op.get("mode").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(
                        target_staging_dir,
                        Path::new(&dest_rel)
                    ) {
                        return Err(anyhow!(
                            "Path traversal detected in zchmod path: \
                             {dest_rel}"
                        ));
                    }

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let full_path = target_staging_dir.join(&dest_rel);
                        fs::set_permissions(
                            full_path,
                            fs::Permissions::from_mode(mode)
                        )?;
                    }
                    if !quiet {
                        println!("Set permissions {mode} on '{dest_rel}'");
                    }
                }
                "zchown" => {
                    let path: String =
                        op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                    let owner: String =
                        op.get("owner").map_err(|e| anyhow!(e.to_string()))?;
                    let group: String =
                        op.get("group").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(
                        target_staging_dir,
                        Path::new(&dest_rel)
                    ) {
                        return Err(anyhow!(
                            "Path traversal detected in zchown path: \
                             {dest_rel}"
                        ));
                    }

                    #[cfg(unix)]
                    {
                        let full_path = target_staging_dir.join(&dest_rel);
                        utils::set_path_owner(&full_path, &owner, &group)?;
                    }
                    if !quiet {
                        println!(
                            "Set ownership {owner}:{group} on '{dest_rel}'"
                        );
                    }
                }
                "zmkdir" => {
                    let path: String =
                        op.get("path").map_err(|e| anyhow!(e.to_string()))?;

                    let dest_rel = resolve_dest(path);

                    if !utils::is_safe_path(
                        target_staging_dir,
                        Path::new(&dest_rel)
                    ) {
                        return Err(anyhow!(
                            "Path traversal detected in zmkdir path: \
                             {dest_rel}"
                        ));
                    }

                    let full_path = target_staging_dir.join(&dest_rel);
                    fs::create_dir_all(full_path)?;
                    if !quiet {
                        println!("Created directory '{dest_rel}'");
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The sub-packages built when none are requested: every declared
/// sub-package plus the main one, or just the main package when the package
/// is not split.
fn default_subs_to_build(pkg: &types::Package) -> Vec<String> {
    if let Some(subs) = &pkg.sub_packages {
        if subs.contains(&String::new()) || subs.contains(&"main".to_string()) {
            subs.clone()
        } else {
            let mut all_subs = vec![String::new()];
            all_subs.extend(subs.clone());
            all_subs
        }
    } else {
        vec![String::new()]
    }
}

/// Core implementation of the build process for a single platform.
fn build_for_platform(
    package_file: &Path,
    build_type: Option<&str>,
    platform: &str,
    sign_key: Option<&String>,
    sign_mode: types::SignMode,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<&Vec<String>>,
    quiet: bool,
    fakeroot: bool,
    _install_deps: bool,
    _test: bool
) -> Result<()> {
    let pkg_lua_dir = package_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let pkg_lua_dir_str = pkg_lua_dir.to_str().ok_or_else(|| {
        anyhow!("Could not get parent directory of package file")
    })?;
    let pkg_for_meta = zoi_lua::parser::parse_lua_package_for_platform(
        package_file.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {}",
                package_file.display()
            )
        })?,
        platform,
        version_override,
        None,
        quiet
    )?;

    if let Some(allowed_platforms) = &pkg_for_meta.platforms
        && !utils::is_platform_compatible(platform, allowed_platforms)
    {
        if !quiet {
            println!(
                "{} Skipping build for platform {}: package only supports \
                 {allowed_platforms:?}",
                "::".bold().yellow(),
                platform.cyan(),
            );
        }
        return Ok(());
    }

    let Some(resolved_build_type) = resolve_build_type(
        build_type,
        &pkg_for_meta.types,
        &pkg_for_meta.name
    )?
    else {
        if !quiet {
            println!(
                "{} Skipping build for package '{}': no build types supported \
                 (likely a collection or template).",
                "::".bold().yellow(),
                pkg_for_meta.name
            );
        }
        return Ok(());
    };

    let version = if let Some(v) = version_override {
        v.to_string()
    } else {
        resolve::get_default_version(&pkg_for_meta, None)?
    };

    let build_dir = Builder::new()
        .prefix(&format!("zoi-build-{}-{platform}", pkg_for_meta.name))
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

    // Stripping only understands ELF, and needs the host's binutils, so it
    // runs for Linux targets built on Linux.
    let debug_config = pkg_for_meta.debug.clone().unwrap_or_default();
    let strip_enabled = cfg!(target_os = "linux")
        && platform.starts_with("linux")
        && debug_config.strip != StripMode::None;
    let debug_sub = strip_enabled && debug_config.sub_package;
    if debug_config.sub_package {
        if pkg_for_meta
            .sub_packages
            .as_ref()
            .is_some_and(|subs| subs.iter().any(|s| s == DEBUG_SUB_PACKAGE))
        {
            return Err(anyhow!(
                "debug.sub_package = true generates the '{DEBUG_SUB_PACKAGE}' \
                 sub-package, but metadata.sub_packages already declares one. \
                 Rename that sub-package or disable debug.sub_package."
            ));
        }
        if debug_config.strip == StripMode::None && !quiet {
            println!(
                "{} debug.sub_package is ignored because debug.strip is \
                 \"none\": nothing is stripped, so there is no debug \
                 information to split off.",
                "::".bold().yellow()
            );
        }
    }
    // Split-off debug information, one staging directory per scope, shared
    // by all sub-packages: a split package gets a single `:debug`.
    let mut debug_stagings: BTreeMap<Scope, tempfile::TempDir> =
        BTreeMap::new();

    let subs_to_build = if let Some(subs) = sub_packages {
        // `:debug` is generated from the other sub-packages, never built by
        // running package() for it.
        let subs: Vec<String> = subs
            .iter()
            .filter(|s| !(debug_sub && s.as_str() == DEBUG_SUB_PACKAGE))
            .cloned()
            .collect();
        if subs.is_empty() {
            default_subs_to_build(&pkg_for_meta)
        } else {
            subs
        }
    } else {
        default_subs_to_build(&pkg_for_meta)
    };

    let scopes_to_process = pkg_for_meta.scopes.clone().unwrap_or(vec![
        Scope::User,
        Scope::System,
        Scope::Project,
    ]);

    let lua_code = fs::read_to_string(package_file)?;

    for sub_package in &subs_to_build {
        let sub_pkg_name = if sub_package.is_empty() {
            None
        } else {
            Some(sub_package.as_str())
        };

        if !quiet && let Some(sub) = sub_pkg_name {
            println!(
                "{} Building sub-package: {}",
                "::".bold().blue(),
                sub.cyan()
            );
        }

        // Shared prepare and build for this sub-package
        {
            let lua_sub = Lua::new();
            zoi_lua::functions::setup_lua_environment(
                &lua_sub,
                platform,
                Some(&version),
                package_file.to_str(),
                None,
                Some(build_dir.path().to_str().unwrap_or("")),
                None, // No staging dir for shared build
                sub_pkg_name,
                Some(pkg_for_meta.scope),
                Some(resolved_build_type.as_str()),
                quiet
            )
            .map_err(|e| anyhow!(e.to_string()))?;

            lua_sub
                .load(&lua_code)
                .exec()
                .map_err(|e| anyhow!(e.to_string()))?;

            let args_sub =
                lua_sub.create_table().map_err(|e| anyhow!(e.to_string()))?;
            if let Some(sub) = sub_pkg_name {
                args_sub
                    .set("sub", sub)
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            let is_prepare_quiet =
                pkg_for_meta.is_stage_quiet("prepare", quiet);
            zoi_lua::api::system::set_session_quiet(&lua_sub, is_prepare_quiet);

            if !skip_prepare
                && let Ok(prepare_fn) =
                    lua_sub.globals().get::<mlua::Function>("prepare")
            {
                if !is_prepare_quiet {
                    println!("Running prepare()...");
                }
                prepare_fn
                    .call::<()>(args_sub.clone())
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            let is_build_quiet = pkg_for_meta.is_stage_quiet("build", quiet);
            zoi_lua::api::system::set_session_quiet(&lua_sub, is_build_quiet);

            if let Ok(build_fn) =
                lua_sub.globals().get::<mlua::Function>("build")
            {
                if !is_build_quiet {
                    println!("Running build()...");
                }
                build_fn
                    .call::<()>(args_sub)
                    .map_err(|e| anyhow!(e.to_string()))?;
            }
        }

        let mut sub_mapping = SubPackageMapping {
            scopes: BTreeMap::new()
        };

        for scope in &scopes_to_process {
            if !quiet {
                println!(
                    "  {} Staging for scope: {scope:?}",
                    "::".bold().blue()
                );
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
                Some(resolved_build_type.as_str()),
                quiet
            )
            .map_err(|e| anyhow!(e.to_string()))?;

            let pkg_table = lua
                .to_value(&pkg_for_meta)
                .map_err(|e| anyhow!(e.to_string()))?;
            lua.globals()
                .set("PKG", pkg_table)
                .map_err(|e| anyhow!(e.to_string()))?;

            lua.load(&lua_code)
                .exec()
                .map_err(|e| anyhow!(e.to_string()))?;

            let args =
                lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
            if !sub_package.is_empty() {
                args.set("sub", sub_package.clone())
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            let is_package_quiet =
                pkg_for_meta.is_stage_quiet("package", quiet);
            zoi_lua::api::system::set_session_quiet(&lua, is_package_quiet);

            if let Ok(package_fn) =
                lua.globals().get::<mlua::Function>("package")
            {
                if !is_package_quiet {
                    println!("Running package()...");
                }
                package_fn
                    .call::<()>(args.clone())
                    .map_err(|e| anyhow!(e.to_string()))?;
            }

            process_build_operations(
                &lua,
                sub_package,
                pkg_lua_dir_str,
                build_dir.path(),
                v_staging.path(),
                true
            )?;

            // Relocate ELFs before pooling so hashes and sizes are recorded
            // from the final relocated bytes. If we relocated after pooling,
            // the manifest sizes/hashes would refer to pre-relocation content
            // and installation would fail its integrity checks.
            if platform.starts_with("linux")
                && let Err(e) =
                    super::relocate::relocate_elfs(v_staging.path(), quiet)
            {
                eprintln!(
                    "{} Failed to relocate ELF binaries in staging: {e}",
                    "Warning:".yellow(),
                );
            }

            // Strip after relocation (which rewrites the dynamic section)
            // and before pooling, so the archive records the final bytes.
            if strip_enabled {
                let debug_dir = if debug_sub {
                    let dir = match debug_stagings.entry(*scope) {
                        std::collections::btree_map::Entry::Occupied(e) => {
                            e.into_mut()
                        }
                        std::collections::btree_map::Entry::Vacant(e) => e
                            .insert(
                                Builder::new()
                                    .prefix("zoi-debugstage-")
                                    .tempdir()?
                            )
                    };
                    Some(dir.path().to_path_buf())
                } else {
                    None
                };
                super::strip::strip_staging(
                    v_staging.path(),
                    &debug_config,
                    debug_dir.as_deref(),
                    is_package_quiet
                )?;
            }

            let mut scope_mapping = ScopeMapping::default();
            super::pool::pool_files(
                v_staging.path(),
                &pool_dir,
                &mut pool,
                &mut scope_mapping,
                fakeroot
            )?;

            sub_mapping.scopes.insert(*scope, scope_mapping);

            if *scope == pkg_for_meta.scope {
                // Run verify and test only for default scope to ensure sanity
                if let Ok(verify_fn) =
                    lua.globals().get::<mlua::Function>("verify")
                {
                    let verification_passed: bool = match verify_fn
                        .call::<mlua::Value>(args.clone())
                    {
                        Ok(mlua::Value::Boolean(b)) => b,
                        Ok(_) => true, // Legacy behavior
                        Err(e) => {
                            return Err(anyhow!("Verification failed: {e}"));
                        }
                    };
                    if !verification_passed {
                        return Err(anyhow!("Package verification failed."));
                    }
                }
            }
        }
        mappings.insert(sub_package.clone(), sub_mapping);
    }

    let output_base = if let Some(dir) = output_dir {
        dir.to_path_buf()
    } else {
        package_file
            .parent()
            .ok_or_else(|| {
                anyhow!("package_file should have a parent directory")
            })?
            .to_path_buf()
    };

    let is_split_package = pkg_for_meta
        .sub_packages
        .as_ref()
        .is_some_and(|subs| !subs.is_empty());

    if is_split_package {
        // Build an independent .zpa archive for each sub-package in
        // subs_to_build
        for sub_package in &subs_to_build {
            let sub_id = if sub_package.is_empty() {
                "main"
            } else {
                sub_package.as_str()
            };

            let Some(sub_mapping) = mappings.get(sub_package) else {
                continue;
            };

            let output_filename = format!(
                "{}-{version}-{platform}.{sub_id}.zpa",
                pkg_for_meta.name
            );
            write_sub_archive(
                sub_package,
                sub_mapping,
                &pool,
                &pool_dir,
                package_file,
                &output_base.join(output_filename),
                fakeroot,
                sign_key.map(std::string::String::as_str),
                sign_mode,
                quiet
            )?;
        }
    } else {
        let pooled_manifest = PooledZpaManifest {
            version: "2".to_string(),
            pool,
            mappings
        };

        let manifest_json =
            pooled_manifest_json(&pooled_manifest, package_file)?;
        fs::write(staging_dir.join("manifest.json"), manifest_json)?;

        fs::copy(
            package_file,
            staging_dir.join(
                package_file.file_name().ok_or_else(|| {
                    anyhow!("package_file should have a name")
                })?
            )
        )?;

        let output_filename =
            format!("{}-{version}-{platform}.zpa", pkg_for_meta.name);
        let output_path = output_base.join(output_filename);

        package_single_zpa(
            &output_path,
            &staging_dir,
            &pooled_manifest,
            fakeroot,
            sign_key.map(std::string::String::as_str),
            sign_mode,
            quiet
        )?;
    }

    if debug_sub {
        // The debug files live in their own pool so they never end up in the
        // main archive of a non-split package.
        let debug_pool_dir = build_dir.path().join("debug-pool");
        fs::create_dir_all(&debug_pool_dir)?;
        let mut debug_pool: BTreeMap<String, PoolFileEntry> = BTreeMap::new();
        let mut debug_mapping = SubPackageMapping {
            scopes: BTreeMap::new()
        };
        for (scope, dir) in &debug_stagings {
            let mut scope_mapping = ScopeMapping::default();
            super::pool::pool_files(
                dir.path(),
                &debug_pool_dir,
                &mut debug_pool,
                &mut scope_mapping,
                fakeroot
            )?;
            debug_mapping.scopes.insert(*scope, scope_mapping);
        }

        if debug_pool.is_empty() {
            if !quiet {
                println!(
                    "{} No debug information found; not creating the '{}' \
                     sub-package.",
                    "::".bold().yellow(),
                    DEBUG_SUB_PACKAGE
                );
            }
        } else {
            let output_filename = format!(
                "{}-{version}-{platform}.{DEBUG_SUB_PACKAGE}.zpa",
                pkg_for_meta.name
            );
            write_sub_archive(
                DEBUG_SUB_PACKAGE,
                &debug_mapping,
                &debug_pool,
                &debug_pool_dir,
                package_file,
                &output_base.join(output_filename),
                fakeroot,
                sign_key.map(std::string::String::as_str),
                sign_mode,
                quiet
            )?;
        }
    }

    Ok(())
}

/// Writes the `.zpa` archive of one sub-package: the pool files its mapping
/// references, a manifest with only its mapping, and the package definition.
///
/// # Errors
///
/// Returns an error if staging, manifest serialization or archive creation
/// fails.
fn write_sub_archive(
    sub_package: &str,
    sub_mapping: &SubPackageMapping,
    pool: &BTreeMap<String, PoolFileEntry>,
    pool_dir: &Path,
    package_file: &Path,
    output_path: &Path,
    fakeroot: bool,
    sign_key: Option<&str>,
    sign_mode: types::SignMode,
    quiet: bool
) -> Result<()> {
    let sub_id = if sub_package.is_empty() {
        "main"
    } else {
        sub_package
    };

    // Collect only the pool files needed by this specific sub-package
    let mut sub_pool: BTreeMap<String, PoolFileEntry> = BTreeMap::new();
    for scope_mapping in sub_mapping.scopes.values() {
        for file in &scope_mapping.files {
            if let Some(entry) = pool.get(&file.hash) {
                sub_pool.insert(file.hash.clone(), entry.clone());
            }
        }
    }

    let sub_staging = Builder::new()
        .prefix(&format!("zoi-stage-{sub_id}-"))
        .tempdir()?;
    let sub_pool_dir = sub_staging.path().join("pool");
    fs::create_dir_all(&sub_pool_dir)?;

    for hash in sub_pool.keys() {
        let src = pool_dir.join(hash);
        let dst = sub_pool_dir.join(hash);
        if src.exists() {
            fs::copy(&src, &dst)?;
        }
    }

    let mut sub_mappings = BTreeMap::new();
    sub_mappings.insert(sub_package.to_string(), sub_mapping.clone());
    if sub_package == "main" {
        sub_mappings.insert(String::new(), sub_mapping.clone());
    } else if sub_package.is_empty() {
        sub_mappings.insert("main".to_string(), sub_mapping.clone());
    }

    let sub_manifest = PooledZpaManifest {
        version: "2".to_string(),
        pool: sub_pool,
        mappings: sub_mappings
    };

    let manifest_json = pooled_manifest_json(&sub_manifest, package_file)?;
    fs::write(sub_staging.path().join("manifest.json"), manifest_json)?;

    fs::copy(
        package_file,
        sub_staging.path().join(
            package_file
                .file_name()
                .ok_or_else(|| anyhow!("package_file should have a name"))?
        )
    )?;

    package_single_zpa(
        output_path,
        sub_staging.path(),
        &sub_manifest,
        fakeroot,
        sign_key,
        sign_mode,
        quiet
    )
}

/// Packages a staging directory into a single `.zpa` archive, including
/// optional embedded or detached PGP signatures, and writing the `.hash`,
/// `.size`, and `.files` sidecar files.
///
/// # Errors
///
/// Returns an error if archive creation, file reading, compression, hashing,
/// or signing operations fail.
fn package_single_zpa(
    output_path: &Path,
    staging_dir: &Path,
    pooled_manifest: &PooledZpaManifest,
    fakeroot: bool,
    sign_key: Option<&str>,
    sign_mode: types::SignMode,
    quiet: bool
) -> Result<()> {
    if let Some(key_id) = sign_key
        && sign_mode == types::SignMode::Embed
        && !quiet
    {
        println!("Signing embedded manifest with key '{}'...", key_id.cyan());
    }
    if let Some(key_id) = sign_key
        && sign_mode == types::SignMode::Embed
    {
        zoi_core::pgp::sign_detached(
            &staging_dir.join("manifest.json"),
            &staging_dir.join("manifest.sig"),
            key_id
        )?;
    }

    {
        let file = File::create(output_path)?;
        let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
        let mut tar_builder = TarBuilder::new(encoder);

        if fakeroot {
            if !quiet {
                println!(
                    "{} Applying fakeroot (UID/GID 0) to archive...",
                    "::".bold().blue()
                );
            }
            for entry in WalkDir::new(staging_dir).min_depth(1) {
                let entry = entry?;
                let path = entry.path();
                let rel_path = path.strip_prefix(staging_dir)?;

                let mut header = tar::Header::new_gnu();
                let metadata = fs::symlink_metadata(path)?;

                header.set_metadata(&metadata);
                header.set_uid(0);
                header.set_gid(0);
                header.set_username("root")?;
                header.set_groupname("root")?;

                if metadata.is_dir() {
                    tar_builder.append_data(
                        &mut header,
                        rel_path,
                        std::io::empty()
                    )?;
                } else if metadata.is_symlink() {
                    let target = fs::read_link(path)?;
                    tar_builder.append_link(&mut header, rel_path, target)?;
                } else {
                    let mut file = File::open(path)?;
                    tar_builder.append_data(
                        &mut header,
                        rel_path,
                        &mut file
                    )?;
                }
            }
        } else {
            tar_builder.append_dir_all(".", staging_dir)?;
        }
        tar_builder.finish()?;
    }

    // Legacy metadata files for compatibility
    let mut files_list = std::collections::HashSet::new();

    // Include the actual destination paths for all sub-packages and scopes
    for sub_pkg in pooled_manifest.mappings.values() {
        for scope_mapping in sub_pkg.scopes.values() {
            for f in &scope_mapping.files {
                files_list.insert(f.dest.clone());
            }
            for s in &scope_mapping.symlinks {
                files_list.insert(s.link.clone());
            }
            for d in &scope_mapping.dirs {
                if d.path == "${pkgstore}"
                    || d.path == "${usrroot}"
                    || d.path == "${usrhome}"
                    || d.path == "${createpkgdir}"
                {
                    continue;
                }
                // Ensure directories end with / to distinguish them in search
                let mut dir_path = d.path.clone();
                if !dir_path.ends_with('/') {
                    dir_path.push('/');
                }
                files_list.insert(dir_path);
            }
        }
    }

    let mut sorted_files: Vec<_> = files_list.into_iter().collect();
    sorted_files.sort();

    let files_manifest_path =
        PathBuf::from(format!("{}.files", output_path.display()));
    fs::write(&files_manifest_path, sorted_files.join("\n"))?;

    let hash_path = PathBuf::from(format!("{}.hash", output_path.display()));
    let output_path_str = output_path.to_str().ok_or_else(|| {
        anyhow!(
            "Output path contains invalid UTF-8: {}",
            output_path.display()
        )
    })?;
    let hash = zoi_core::hash::calculate_file_hash(
        Path::new(output_path_str),
        zoi_core::hash::HashAlgorithm::Sha512
    )?;
    fs::write(
        &hash_path,
        format!(
            "{hash}  {}\n",
            output_path
                .file_name()
                .ok_or_else(|| anyhow!("output_path should have a name"))?
                .to_str()
                .ok_or_else(|| anyhow!(
                    "output_filename should be valid UTF-8"
                ))?
        )
    )?;

    let size_path = PathBuf::from(format!("{}.size", output_path.display()));
    let compressed_size = fs::metadata(output_path)?.len();
    let uncompressed_size: u64 = WalkDir::new(staging_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map_or(0, |m| m.len()))
        .sum();
    fs::write(
        &size_path,
        format!("down: {compressed_size}\ninstall: {uncompressed_size}\n")
    )?;

    if !quiet {
        println!(
            "{}",
            format!("Successfully built package: {}", output_path.display())
                .green()
        );
    }

    if let Some(key_id) = sign_key {
        match sign_mode {
            types::SignMode::Embed => {
                if !quiet {
                    println!("{}", "Embedded signature created.".green());
                }
            }
            types::SignMode::File => {
                if !quiet {
                    println!("Signing package with key '{}'...", key_id.cyan());
                }
                let signature_path =
                    PathBuf::from(format!("{}.sig", output_path.display()));
                if signature_path.exists() {
                    fs::remove_file(&signature_path)?;
                }
                zoi_core::pgp::sign_detached(
                    output_path,
                    &signature_path,
                    key_id
                )?;
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
        }
    }

    Ok(())
}

/// Executes the build process for one or more platforms.
///
/// # Errors
///
/// Returns an error if:
/// - A .zsa bundle cannot be extracted.
/// - The required image is not specified for Docker builds.
/// - Building for 'all' platforms is requested.
/// - One or more platform builds fail.
pub fn run(
    package_file: &Path,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    sign_mode: types::SignMode,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    quiet: bool,
    method: &str,
    image: Option<&str>,
    fakeroot: bool,
    install_deps: bool,
    test: bool
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
            default_output_dir = package_file.parent().map(Path::to_path_buf);
        }

        let temp_dir = Builder::new().prefix("zoi-zsa-extract-").tempdir()?;
        let file = File::open(package_file)?;
        let decoder = ZstdDecoder::new(file)?;
        let mut archive = Archive::new(decoder);
        archive.unpack(temp_dir.path())?;

        // Locate the .pkg.lua file inside the bundle. The top-level (and,
        // when present, signed) definition is preferred so the file built is
        // the one the installer verified; a nested search only remains as a
        // fallback for bundles that carry no top-level definition.
        let mut pkg_lua =
            super::bundle::select_bundle_pkg_lua(temp_dir.path())?;
        if pkg_lua.is_none() {
            for entry in WalkDir::new(temp_dir.path())
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_name().to_string_lossy().ends_with(".pkg.lua") {
                    pkg_lua = Some(entry.path().to_path_buf());
                    break;
                }
            }
        }

        actual_package_file = pkg_lua.ok_or_else(|| {
            anyhow!("Could not find .pkg.lua file inside the .zsa bundle.")
        })?;
        _temp_zsa_dir = Some(temp_dir);
    }

    let package_file = actual_package_file.as_path();
    let output_dir = output_dir.or(default_output_dir.as_deref());

    if method == "docker" {
        let docker_image = image.ok_or_else(|| {
            anyhow!(
                "An image must be specified when using the 'docker' build \
                 method."
            )
        })?;
        return super::docker::run(
            package_file,
            build_type,
            platforms,
            sign_key,
            sign_mode,
            output_dir,
            version_override,
            sub_packages,
            docker_image,
            fakeroot,
            install_deps,
            test
        );
    }

    if method == "bwrap" {
        return super::bwrap::run(
            package_file,
            build_type,
            platforms,
            sign_key,
            sign_mode,
            output_dir,
            version_override,
            sub_packages,
            fakeroot,
            install_deps,
            test
        );
    }

    if !quiet {
        println!("Building package from: {}", package_file.display());
    }

    let platforms_to_build: Vec<String> =
        if platforms.contains(&"current".to_string()) {
            let mut p = platforms.to_vec();
            p.retain(|x| x != "current");
            p.push(utils::get_platform()?);
            p
        } else {
            platforms.to_vec()
        };

    if platforms.contains(&"all".to_string()) {
        return Err(anyhow!(
            "Building for 'all' platforms is not supported in this flow yet. \
             Please specify platforms explicitly."
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
            sign_key.as_ref(),
            sign_mode,
            output_dir,
            version_override,
            sub_packages.as_ref(),
            quiet,
            fakeroot,
            install_deps,
            test
        ) {
            eprintln!(
                "{}: Failed to build for platform {}: {e}",
                "Error".red().bold(),
                platform.red(),
            );
            any_failed = true;
        }
    }

    if any_failed {
        return Err(anyhow!("One or more platform builds failed"));
    }

    Ok(())
}
