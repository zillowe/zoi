use anyhow::{Result, anyhow};
use colored::*;
use ignore::gitignore::GitignoreBuilder;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Builder as TarBuilder;
use tempfile::Builder;
use walkdir::WalkDir;
use zstd::stream::write::Encoder as ZstdEncoder;

pub fn run(
    package_file: &Path,
    output_dir: Option<&Path>,
    sign: Option<String>,
    version_override: Option<&str>,
    build_type: Option<String>,
) -> Result<()> {
    let pkg_dir = package_file
        .parent()
        .ok_or_else(|| anyhow!("Could not get parent directory"))?;

    // Load .zoiignore if it exists
    let mut ignore_builder = GitignoreBuilder::new(pkg_dir);
    let zoiignore_path = pkg_dir.join(".zoiignore");
    if zoiignore_path.exists()
        && let Some(err) = ignore_builder.add(&zoiignore_path)
    {
        eprintln!("{}: Error parsing .zoiignore: {}", "Warning".yellow(), err);
    }
    let ignore = ignore_builder.build()?;

    let is_ignored = |rel_path: &Path| -> bool {
        // ignore-rs expectations: directories should have a trailing slash or be explicitly marked
        // but here we just check the path.
        ignore.matched(rel_path, rel_path.is_dir()).is_ignore()
    };

    println!(
        "{} Bundling package: {}",
        "::".bold().blue(),
        package_file.display()
    );

    let lua = Lua::new();
    let platform = zoi_core::utils::get_platform()?;

    // Initialize global tables for tracking
    let refs_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiReferencedFiles", refs_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Setup a mocked environment for metadata and asset discovery
    // We run it twice: once to find local assets, and once to actually run prepare() if needed.

    let bundle_type = build_type.as_deref().unwrap_or("source");

    // Phase 1: Metadata & Local Asset Discovery
    zoi_lua::functions::setup_lua_environment(
        &lua,
        &platform,
        version_override,
        package_file.to_str(),
        None,
        Some("/tmp/mock-build"),
        Some("/tmp/mock-staging"),
        None,
        None,
        Some(bundle_type),
        true, // quiet
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    lua.globals()
        .set("BUILD_TYPE", bundle_type)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Mock UTILS.EXTRACT to record local references but avoid downloads (in this phase)
    if let Ok(utils) = lua.globals().get::<Table>("UTILS") {
        let mock_extract = lua
            .create_function(|_, (_source, _out_dir): (String, String)| Ok(()))
            .map_err(|e| anyhow!(e.to_string()))?;
        utils
            .set("EXTRACT", mock_extract)
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    // Mock cmd to avoid shell execution in this phase
    let mock_cmd = lua
        .create_function(|_, _command: String| Ok((String::new(), String::new(), 0)))
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("cmd", mock_cmd)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Load and execute the package file
    let lua_code = fs::read_to_string(package_file)?;
    lua.load(&lua_code).exec().map_err(|e| {
        anyhow!(
            "Failed to execute Lua package file '{}' for bundling:\n{}",
            package_file.display(),
            e
        )
    })?;

    let args = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;

    // Call lifecycle functions to find ${pkgluadir} references
    if let Ok(pkg_fn) = lua.globals().get::<mlua::Function>("package") {
        let _ = pkg_fn.call::<()>(args.clone());
    }

    let mut files_to_include = HashSet::new();

    // Always include the package file itself
    let pkg_filename = package_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid package file"))?;
    files_to_include.insert(pkg_filename.to_string_lossy().to_string());

    // Collect from __ZoiReferencedFiles (IMPORT/INCLUDE)
    if let Ok(refs) = lua.globals().get::<Table>("__ZoiReferencedFiles") {
        for val in refs.sequence_values::<String>() {
            files_to_include.insert(val.map_err(|e| anyhow!(e.to_string()))?);
        }
    }

    // Collect from __ZoiBuildOperations (zcp/zln with ${pkgluadir})
    if let Ok(ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in ops.sequence_values::<Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;

            // Check 'source' (used by zcp)
            if let Ok(source) = op.get::<String>("source")
                && let Some(rel) = source.strip_prefix("${pkgluadir}/")
            {
                files_to_include.insert(rel.to_string());
            }

            // Check 'target' (used by zln)
            if let Ok(target) = op.get::<String>("target")
                && let Some(rel) = target.strip_prefix("${pkgluadir}/")
            {
                files_to_include.insert(rel.to_string());
            }
        }
    }

    // Phase 2: Fetching Upstream Sources (Running prepare)
    println!("{} Fetching upstream sources...", "::".bold().blue());
    let fetch_dir = Builder::new().prefix("zoi-bundle-fetch-").tempdir()?;

    // Setup a real environment for prepare()
    let lua_fetch = Lua::new();

    // Initialize package metadata tables for the fetch state
    let pkg_meta_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    let pkg_deps_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    let pkg_updates_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    let pkg_hooks_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    let pkg_service_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("__ZoiPackageMeta", pkg_meta_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("__ZoiPackageDeps", pkg_deps_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("__ZoiPackageUpdates", pkg_updates_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("__ZoiPackageHooks", pkg_hooks_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("__ZoiPackageService", pkg_service_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;

    let pkg_global_table_f = lua_fetch
        .create_table()
        .map_err(|e| anyhow!(e.to_string()))?;
    lua_fetch
        .globals()
        .set("PKG", pkg_global_table_f)
        .map_err(|e| anyhow!(e.to_string()))?;

    zoi_lua::functions::setup_lua_environment(
        &lua_fetch,
        &platform,
        version_override,
        package_file.to_str(),
        None,
        Some(fetch_dir.path().to_str().unwrap_or("")),
        Some("/tmp/mock-staging"),
        None,
        None,
        Some(bundle_type),
        true, // quiet
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    lua_fetch
        .globals()
        .set("BUILD_TYPE", bundle_type)
        .map_err(|e| anyhow!(e.to_string()))?;

    lua_fetch
        .globals()
        .set(
            "BUILD_DIR",
            fetch_dir
                .path()
                .to_str()
                .ok_or_else(|| anyhow!("Invalid fetch path"))?,
        )
        .map_err(|e| anyhow!(e.to_string()))?;

    // We use the real cmd implementation for fetching
    zoi_lua::api::system::add_cmd_util(&lua_fetch, true).map_err(|e| anyhow!(e.to_string()))?;

    // Reload script in the fetch environment
    lua_fetch.load(&lua_code).exec().map_err(|e| {
        anyhow!(
            "Failed to execute Lua package file '{}' during fetch:\n{}",
            package_file.display(),
            e
        )
    })?;

    if let Ok(prep_fn) = lua_fetch.globals().get::<mlua::Function>("prepare") {
        println!("  Running prepare()...");
        let args_fetch = lua_fetch
            .create_table()
            .map_err(|e| anyhow!(e.to_string()))?;
        prep_fn.call::<()>(args_fetch).map_err(|e| {
            anyhow!(
                "The 'prepare' function in '{}' failed during bundling:\n{}",
                package_file.display(),
                e
            )
        })?;
    }

    // Determine output path
    let pkg_dir = package_file
        .parent()
        .ok_or_else(|| anyhow!("Could not get parent directory"))?;

    let final_pkg_meta: Table = lua
        .globals()
        .get("__ZoiPackageMeta")
        .map_err(|e| anyhow!(e.to_string()))?;
    let pkg_meta: zoi_core::types::Package = lua
        .from_value(Value::Table(final_pkg_meta))
        .map_err(|e| anyhow!(e.to_string()))?;

    let version = version_override
        .map(|v| v.to_string())
        .or(pkg_meta.version)
        .unwrap_or_else(|| "unknown".to_string());
    let output_filename = format!("{}-{}.zsa", pkg_meta.name, version);
    let output_base = output_dir
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| pkg_dir.to_path_buf());
    let output_path = output_base.join(output_filename);

    let file = File::create(&output_path)?;
    let encoder = ZstdEncoder::new(file, 0)?.auto_finish();
    let mut tar_builder = TarBuilder::new(encoder);

    // Include local files
    let mut sorted_files: Vec<_> = files_to_include.into_iter().collect();
    sorted_files.sort();

    for rel_path_str in sorted_files {
        let rel_path = Path::new(&rel_path_str);
        if is_ignored(rel_path) {
            println!("  Ignored: {}", rel_path_str);
            continue;
        }

        let abs_path = pkg_dir.join(rel_path);
        if abs_path.exists() {
            if abs_path.is_dir() {
                tar_builder.append_dir_all(&rel_path_str, &abs_path)?;
            } else {
                tar_builder.append_path_with_name(&abs_path, &rel_path_str)?;
            }
            println!("  Included local: {}", rel_path_str);
        }
    }

    // Include fetched files from BUILD_DIR
    for entry in WalkDir::new(fetch_dir.path()).min_depth(1) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(fetch_dir.path())?;
        let rel_path_str = rel_path.to_string_lossy();

        if is_ignored(rel_path) {
            println!("  Ignored fetch: {}", rel_path_str);
            continue;
        }

        if entry.file_type().is_dir() {
            // We'll add directories as we encounter their files or empty dirs
            continue;
        }

        tar_builder.append_path_with_name(entry.path(), rel_path)?;
        println!("  Included fetch: {}", rel_path_str);
    }

    // Mark as a full bundle so build knows to skip prepare
    let mut header = tar::Header::new_gnu();
    header.set_path(".zoi-prepared")?;
    header.set_size(0);
    header.set_cksum();
    tar_builder.append(&header, &[][..])?;

    tar_builder.finish()?;
    println!(
        "{} Successfully created bundle: {}",
        "::".bold().green(),
        output_path.display()
    );

    if let Some(key_id) = sign {
        println!(
            "{} Signing bundle with key '{}'...",
            "::".bold().blue(),
            key_id.cyan()
        );
        let signature_path = PathBuf::from(format!("{}.sig", output_path.display()));
        if signature_path.exists() {
            fs::remove_file(&signature_path)?;
        }
        zoi_core::pgp::sign_detached(&output_path, &signature_path, &key_id)?;
        println!(
            "{} Successfully created signature: {}",
            "::".bold().green(),
            signature_path.display()
        );
    }

    Ok(())
}
