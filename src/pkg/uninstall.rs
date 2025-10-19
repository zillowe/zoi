use crate::pkg::{dependencies, hooks, local, recorder, resolve, types};
use crate::utils;
use anyhow::anyhow;
use colored::*;
use mlua::Lua;
use std::fs;
use std::path::PathBuf;

fn get_bin_root(scope: types::Scope) -> anyhow::Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir =
                home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(home_dir.join(".zoi/pkgs/bin"))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(PathBuf::from("C:\\ProgramData\\zoi\\pkgs\\bin"))
            } else {
                Ok(PathBuf::from("/usr/local/bin"))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

fn uninstall_collection(
    pkg: &types::Package,
    manifest: &types::InstallManifest,
    scope: types::Scope,
    registry_handle: Option<String>,
) -> anyhow::Result<types::InstallManifest> {
    println!("Uninstalling collection '{}'...", pkg.name.bold());

    let dependencies_to_uninstall = &manifest.installed_dependencies;

    if dependencies_to_uninstall.is_empty() {
        println!("Collection has no dependencies to uninstall.");
    } else {
        println!("Uninstalling dependencies of the collection...");
        for dep_str in dependencies_to_uninstall {
            println!("\n--- Uninstalling dependency: {} ---", dep_str.bold());
            if let Err(e) = dependencies::uninstall_dependency(dep_str, &move |name| {
                run(name, Some(scope)).map(|_| ())
            }) {
                eprintln!(
                    "Warning: Could not uninstall dependency '{}': {}",
                    dep_str, e
                );
            }
        }
    }

    let handle = registry_handle.as_deref().unwrap_or("local");
    let package_dir = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)?;
    }
    if let Err(e) = recorder::remove_package_from_record(&pkg.name, scope) {
        eprintln!(
            "{} Failed to remove package from lockfile: {}",
            "Warning:".yellow(),
            e
        );
    }

    match crate::pkg::telemetry::posthog_capture_event("uninstall", pkg, env!("CARGO_PKG_VERSION"))
    {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    Ok(manifest.clone())
}

pub fn run(
    package_name: &str,
    scope_override: Option<types::Scope>,
) -> anyhow::Result<types::InstallManifest> {
    let (pkg, _, _, pkg_lua_path, registry_handle) =
        resolve::resolve_package_and_version(package_name)?;

    let (manifest, scope) = if let Some(scope) = scope_override {
        if let Some(m) = local::is_package_installed(&pkg.name, scope)? {
            (m, scope)
        } else {
            return Err(anyhow::anyhow!(
                "Package '{}' is not installed in the specified scope.",
                package_name
            ));
        }
    } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::Project)? {
        (m, types::Scope::Project)
    } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::User)? {
        (m, types::Scope::User)
    } else if let Some(m) = local::is_package_installed(&pkg.name, types::Scope::System)? {
        (m, types::Scope::System)
    } else {
        return Err(anyhow::anyhow!(
            "Package '{}' is not installed by Zoi.",
            package_name
        ));
    };

    if pkg.package_type == types::PackageType::Collection {
        return uninstall_collection(&pkg, &manifest, scope, registry_handle);
    }

    if let Some(hooks) = &pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreRemove)
    {
        return Err(anyhow::anyhow!("Pre-remove hook failed: {}", e));
    }

    let handle = registry_handle.as_deref().unwrap_or("local");
    let package_dir = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;
    let dependents = local::get_dependents(&package_dir)?;
    if !dependents.is_empty() {
        return Err(anyhow::anyhow!(
            "Cannot uninstall '{}' because other packages depend on it:\n  -{}\n\nPlease uninstall these packages first.",
            &pkg.name,
            dependents.join("\n  - ")
        ));
    }

    let lua = Lua::new();
    crate::pkg::lua::functions::setup_lua_environment(
        &lua,
        &utils::get_platform()?,
        Some(&manifest.version),
        pkg_lua_path.to_str(),
        None,
    )
    .map_err(|e| anyhow!(e.to_string()))?;
    let lua_code = fs::read_to_string(pkg_lua_path)?;
    lua.load(&lua_code)
        .exec()
        .map_err(|e| anyhow!(e.to_string()))?;

    if let Ok(uninstall_fn) = lua.globals().get::<mlua::Function>("uninstall") {
        println!("Running uninstall() script...");
        uninstall_fn
            .call::<()>(())
            .map_err(|e| anyhow!(e.to_string()))?;
    }

    if let Ok(uninstall_ops) = lua.globals().get::<mlua::Table>("__ZoiUninstallOperations") {
        for op in uninstall_ops.sequence_values::<mlua::Table>() {
            let op = op.map_err(|e| anyhow!(e.to_string()))?;
            if let Ok(op_type) = op.get::<String>("op")
                && op_type == "zrm"
            {
                let path_to_remove: String = op.get("path").map_err(|e| anyhow!(e.to_string()))?;
                let path = std::path::PathBuf::from(path_to_remove);
                if path.exists() {
                    println!("Removing {}...", path.display());
                    if path.is_dir() {
                        fs::remove_dir_all(path)?;
                    } else {
                        fs::remove_file(path)?;
                    }
                }
            }
        }
    }

    println!(
        "Uninstalling '{}' and its unused dependencies...",
        pkg.name.bold()
    );

    if let Some(bins) = &manifest.bins {
        let bin_root = get_bin_root(scope)?;
        for bin in bins {
            let symlink_path = bin_root.join(bin);
            if symlink_path.is_symlink() || symlink_path.exists() {
                println!("Removing symlink from {}...", symlink_path.display());
                fs::remove_file(&symlink_path)?;
                println!("{}", "Successfully removed symlink.".green());
            }
        }
    } else {
        let symlink_path = get_bin_root(scope)?.join(&pkg.name);
        if symlink_path.is_symlink() || symlink_path.exists() {
            println!("Removing symlink from {}...", symlink_path.display());
            fs::remove_file(symlink_path)?;
            println!("{}", "Successfully removed symlink.".green());
        }
    }
    let handle = registry_handle.as_deref().unwrap_or("local");
    let package_dir = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;

    if package_dir.exists() {
        println!("Removing stored files from {}...", package_dir.display());
        fs::remove_dir_all(&package_dir)?;
        println!("{}", "Successfully removed stored files.".green());
    }
    let parent_id = format!("#{}@{}", manifest.registry_handle, manifest.repo);
    for dep_str in &manifest.installed_dependencies {
        if let Ok(dep) = dependencies::parse_dependency_string(dep_str)
            && dep.manager == "zoi"
            && let Ok(Some(dep_manifest)) = local::is_package_installed(dep.package, scope)
        {
            match local::get_package_dir(
                dep_manifest.scope,
                &dep_manifest.registry_handle,
                &dep_manifest.repo,
                &dep_manifest.name,
            ) {
                Ok(dep_pkg_dir) => {
                    if let Err(e) = local::remove_dependent(&dep_pkg_dir, &parent_id) {
                        eprintln!(
                            "Warning: failed to remove dependent link for {}: {}",
                            dep.package, e
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: failed to get package dir for {}: {}",
                        dep.package, e
                    );
                }
            }
        }
    }

    if let Err(e) = recorder::remove_package_from_record(&pkg.name, scope) {
        eprintln!(
            "{} Failed to remove package from lockfile: {}",
            "Warning:".yellow(),
            e
        );
    }
    println!("Removed manifest for '{}'.", pkg.name);

    match crate::pkg::telemetry::posthog_capture_event("uninstall", &pkg, env!("CARGO_PKG_VERSION"))
    {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    if let Some(hooks) = &pkg.hooks
        && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostRemove)
    {
        return Err(anyhow::anyhow!("Post-remove hook failed: {}", e));
    }

    Ok(manifest)
}
