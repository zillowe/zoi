use anyhow::{Result, anyhow};
use colored::*;
use mlua::LuaSerdeExt;
use std::fs;
use std::path::Path;
use tar::Archive;
use tempfile::Builder;
use zoi_core::{types, utils};
use zoi_package as package;
use zoi_plugins::PluginManager;
use zoi_resolver as resolver;
use zstd::stream::read::Decoder as ZstdDecoder;

fn install_app_from_archive(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    println!(
        "Extracting app to '{}'...",
        destination_dir.display().to_string().cyan()
    );
    let file = fs::File::open(archive_path)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);

    let temp_extract_dir = Builder::new().prefix("zoi-create-extract-").tempdir()?;

    archive.unpack(temp_extract_dir.path())?;

    let manifest_path = temp_extract_dir.path().join("manifest.json");
    if !manifest_path.exists() {
        // Fallback to legacy format
        let create_pkg_dir = temp_extract_dir.path().join("data/createpkgdir");
        if !create_pkg_dir.exists() {
            return Err(anyhow!(
                "Archive is not a valid app package: missing 'manifest.json' or legacy 'data/createpkgdir'."
            ));
        }
        utils::copy_dir_all(&create_pkg_dir, destination_dir)?;
        return Ok(());
    }

    // Pooled format
    let content = fs::read_to_string(&manifest_path)?;
    let pooled_manifest = serde_json::from_str::<types::PooledZpaManifest>(&content)?;
    let pool_dir = temp_extract_dir.path().join("pool");

    // App templates usually use the "" sub-package and project scope
    if let Some(sub_mapping) = pooled_manifest.mappings.get("")
        && let Some(scope_mapping) = sub_mapping.scopes.get(&types::Scope::Project)
    {
        for mapped_dir in &scope_mapping.dirs {
            if let Some(rel) = mapped_dir.path.strip_prefix("${createpkgdir}/") {
                fs::create_dir_all(destination_dir.join(rel))?;
            }
        }
        for mapped_file in &scope_mapping.files {
            if let Some(rel) = mapped_file.dest.strip_prefix("${createpkgdir}/") {
                let dest_path = destination_dir.join(rel);
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(pool_dir.join(&mapped_file.hash), &dest_path)?;
            }
        }
        for mapped_link in &scope_mapping.symlinks {
            if let Some(rel) = mapped_link.link.strip_prefix("${createpkgdir}/") {
                let dest_path = destination_dir.join(rel);
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                utils::symlink_file(Path::new(&mapped_link.target), &dest_path)?;
            }
        }
    }

    Ok(())
}

pub fn run(
    source: &str,
    app_name: Option<String>,
    yes: bool,
    plugin_manager: Option<&PluginManager>,
) -> Result<()> {
    let (pkg, _, _, pkg_lua_path, _, _, _) =
        resolver::resolve::resolve_package_and_version(source, None, false, false)?;

    if pkg.package_type != types::PackageType::App {
        return Err(anyhow!(
            "Package '{}' is not of type 'app'. Use 'zoi install' for packages and collections.",
            pkg.name
        ));
    }

    let mut pkg_val = None;
    if let Some(pm) = plugin_manager {
        let v = pm
            .lua
            .to_value(&pkg)
            .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
        pm.trigger_hook("on_pre_create", Some(v.clone()))?;
        pkg_val = Some(v);
    }

    let dest_name = app_name.unwrap_or_else(|| pkg.name.clone());
    let app_dir = Path::new(&dest_name);

    if app_dir.exists() {
        if app_dir.is_dir() {
            if fs::read_dir(app_dir)?.next().is_some() {
                println!(
                    "{}",
                    format!(
                        "Warning: Directory '{}' already exists and is not empty.",
                        dest_name
                    )
                    .yellow()
                );
                if !utils::ask_for_confirmation("Do you want to continue?", yes) {
                    return Err(anyhow!("Operation aborted by user."));
                }
            }
        } else {
            return Err(anyhow!(
                "A file with the name '{}' already exists.",
                dest_name
            ));
        }
    }

    println!(
        "Creating app '{}' using template '{}'...",
        dest_name.cyan(),
        pkg.name.green()
    );

    let build_dir = Builder::new().prefix("zoi-create-build-").tempdir()?;

    package::build::run(
        &pkg_lua_path,
        Some("source"),
        &[utils::get_platform()?],
        None,
        Some(build_dir.path()),
        pkg.version.as_deref(),
        None,
        false,
        "native",
        None,
        false,
        false,
    )?;

    let archive_filename = format!(
        "{}-{}-{}.zpa",
        pkg.name,
        pkg.version.as_deref().unwrap_or_default(),
        utils::get_platform()?,
    );
    let archive_path = build_dir.path().join(archive_filename);

    if !archive_path.exists() {
        return Err(anyhow!("Build failed to produce an archive."));
    }

    install_app_from_archive(&archive_path, app_dir)?;

    if let (Some(pm), Some(v)) = (plugin_manager, pkg_val) {
        pm.trigger_hook_nonfatal("on_post_create", Some(v));
    }

    println!("\n{}", "App created successfully.".green());

    Ok(())
}
