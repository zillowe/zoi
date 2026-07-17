use anyhow::{Result, anyhow};
use colored::*;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::Path;
use tar::Builder as TarBuilder;
use zstd::stream::write::Encoder as ZstdEncoder;

pub fn run(
    package_file: &Path,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
) -> Result<()> {
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

    // Initialize package metadata tables
    let pkg_meta_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_deps_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_updates_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_hooks_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    let pkg_service_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageMeta", pkg_meta_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageDeps", pkg_deps_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageUpdates", pkg_updates_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageHooks", pkg_hooks_table)
        .map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("__ZoiPackageService", pkg_service_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    let pkg_global_table = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
    lua.globals()
        .set("PKG", pkg_global_table)
        .map_err(|e| anyhow!(e.to_string()))?;

    // Setup a mocked environment
    // We want to avoid heavy side effects like actually extracting or running commands
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
        true, // quiet
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    // Mock UTILS.EXTRACT to avoid downloads
    if let Ok(utils) = lua.globals().get::<Table>("UTILS") {
        let mock_extract = lua
            .create_function(|_, (_source, _out_dir): (String, String)| Ok(()))
            .map_err(|e| anyhow!(e.to_string()))?;
        utils
            .set("EXTRACT", mock_extract)
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    // Mock cmd to avoid shell execution
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

    // Optionally call package() if it exists to find ${pkgluadir} references
    if let Ok(pkg_fn) = lua.globals().get::<mlua::Function>("package") {
        let args = lua.create_table().map_err(|e| anyhow!(e.to_string()))?;
        let _ = pkg_fn.call::<()>(args);
    }

    let mut files_to_include = HashSet::new();

    // Always include the package file itself
    let pkg_filename = package_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid package file"))?;
    files_to_include.insert(pkg_filename.to_string_lossy().to_string());

    // 1. Collect from __ZoiReferencedFiles (IMPORT/INCLUDE)
    let refs: Table = lua
        .globals()
        .get("__ZoiReferencedFiles")
        .map_err(|e| anyhow!(e.to_string()))?;
    for val in refs.sequence_values::<String>() {
        files_to_include.insert(val.map_err(|e| anyhow!(e.to_string()))?);
    }

    // 2. Collect from __ZoiBuildOperations (zcp/zln with ${pkgluadir})
    if let Ok(ops) = lua.globals().get::<Table>("__ZoiBuildOperations") {
        for op in ops.sequence_values::<Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;
            if let Ok(source) = op.get::<String>("source") {
                if let Some(rel) = source.strip_prefix("${pkgluadir}/") {
                    files_to_include.insert(rel.to_string());
                } else if source.contains("${pkgluadir}") {
                    // Handle cases like "foo/${pkgluadir}/bar" if anyone does that
                    files_to_include.insert(source.replace("${pkgluadir}/", ""));
                }
            }
        }
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

    let mut sorted_files: Vec<_> = files_to_include.into_iter().collect();
    sorted_files.sort();

    for rel_path_str in sorted_files {
        let abs_path = pkg_dir.join(&rel_path_str);
        if abs_path.exists() {
            if abs_path.is_dir() {
                tar_builder.append_dir_all(&rel_path_str, &abs_path)?;
            } else {
                tar_builder.append_path_with_name(&abs_path, &rel_path_str)?;
            }
            println!("  Included: {}", rel_path_str);
        } else {
            eprintln!(
                "{} Referenced file not found: {}",
                "Warning:".yellow(),
                rel_path_str
            );
        }
    }

    tar_builder.finish()?;
    println!(
        "{} Successfully created bundle: {}",
        "::".bold().green(),
        output_path.display()
    );

    Ok(())
}
