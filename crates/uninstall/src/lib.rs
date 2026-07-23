pub mod autoremove;

use anyhow::anyhow;
use colored::*;
use mlua::Lua;
use std::fs;
use std::path::PathBuf;
use zoi_core::{recorder, sysroot, types, utils as core_utils};
use zoi_db as db;
use zoi_deps as dependencies;
use zoi_hooks as hooks;
use zoi_resolver::{local, resolve};
use zoi_telemetry as telemetry;

fn get_bin_root(scope: types::Scope) -> anyhow::Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir = core_utils::get_user_home()
                .ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(sysroot::apply_sysroot(home_dir.join(".zoi/pkgs/bin")))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(sysroot::apply_sysroot(PathBuf::from(
                    "C:\\ProgramData\\zoi\\pkgs\\bin",
                )))
            } else {
                Ok(sysroot::apply_sysroot(PathBuf::from("/usr/local/bin")))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

fn get_completions_root(scope: types::Scope, shell: &str) -> anyhow::Result<PathBuf> {
    match scope {
        types::Scope::User => {
            let home_dir = core_utils::get_user_home()
                .ok_or_else(|| anyhow!("Could not find home directory."))?;
            Ok(sysroot::apply_sysroot(
                home_dir.join(".zoi/pkgs/shell").join(shell),
            ))
        }
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(sysroot::apply_sysroot(PathBuf::from(format!(
                    "C:\\ProgramData\\zoi\\pkgs\\shell\\{}",
                    shell
                ))))
            } else {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/local/share/zoi/completions",
                };
                Ok(sysroot::apply_sysroot(PathBuf::from(base)))
            }
        }
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir
                .join(".zoi")
                .join("pkgs")
                .join("shell")
                .join(shell))
        }
    }
}

fn cleanup_service(package_name: &str, scope: types::Scope) -> anyhow::Result<()> {
    let service_name = format!("zoi-{}", package_name);
    let is_user = scope != types::Scope::System;

    match std::env::consts::OS {
        "linux" => {
            let unit_path = if is_user {
                let home = core_utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"))?;
                sysroot::apply_sysroot(
                    home.join(".config/systemd/user")
                        .join(format!("{}.service", service_name)),
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/etc/systemd/system/{}.service",
                    service_name
                )))
            };
            if unit_path.exists() {
                println!("Removing service unit file: {}", unit_path.display());
                fs::remove_file(&unit_path).map_err(|e| {
                    anyhow!("Failed to remove unit file: {}: {}", unit_path.display(), e)
                })?;
                if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err() {
                    let mut cmd = std::process::Command::new("systemctl");
                    if is_user {
                        cmd.arg("--user");
                    }
                    cmd.arg("daemon-reload")
                        .status()
                        .map_err(|e| anyhow!("Failed to run systemctl daemon-reload: {}", e))?;
                }
            }
        }
        "macos" => {
            let plist_path = if is_user {
                let home = core_utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"))?;
                sysroot::apply_sysroot(
                    home.join("Library/LaunchAgents")
                        .join(format!("{}.plist", service_name)),
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/Library/LaunchDaemons/{}.plist",
                    service_name
                )))
            };
            if plist_path.exists() {
                println!("Removing service plist file: {}", plist_path.display());
                fs::remove_file(&plist_path).map_err(|e| {
                    anyhow!(
                        "Failed to remove plist file: {}: {}",
                        plist_path.display(),
                        e
                    )
                })?;
            }
        }
        "windows" => {
            let exists = {
                let output = std::process::Command::new("sc")
                    .arg("query")
                    .arg(&service_name)
                    .output()
                    .map_err(|e| anyhow!("Failed to run sc query: {}", e))?;
                output.status.success()
            };
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err() && exists {
                println!("Removing Windows service: {}", service_name);
                std::process::Command::new("sc")
                    .arg("delete")
                    .arg(&service_name)
                    .status()
                    .map_err(|e| anyhow!("Failed to run sc delete: {}", e))?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn uninstall_collection(
    pkg: &types::Package,
    manifest: &types::InstallManifest,
    scope: types::Scope,
    registry_handle: Option<String>,
    yes: bool,
    quiet: bool,
    dry_run: bool,
) -> anyhow::Result<types::InstallManifest> {
    if !quiet {
        println!("Uninstalling collection '{}'...", pkg.name.bold());
    }

    if dry_run {
        return Ok(manifest.clone());
    }

    let dependencies_to_uninstall = &manifest.installed_dependencies;

    if dependencies_to_uninstall.is_empty() {
        if !quiet {
            println!("Collection has no dependencies to uninstall.");
        }
    } else {
        if !quiet {
            println!("Uninstalling dependencies of the collection...");
        }
        for dep_str in dependencies_to_uninstall {
            let dep = dependencies::parse_dependency_string(dep_str)?;

            if dep.manager != "zoi" {
                let prompt = format!(
                    "Uninstall native dependency '{}' ({})?",
                    dep.package.cyan(),
                    dep.manager.yellow()
                );
                let warning = "Warning: Zoi cannot track if other non-Zoi applications depend on this package.";

                if yes {
                    if !quiet {
                        println!(
                            "\n{} Uninstalling native dependency: {}...",
                            "::".bold().blue(),
                            dep_str.bold()
                        );
                        println!("{}: {}", "Note".yellow(), warning);
                    }
                } else if core_utils::ask_for_confirmation(
                    &format!("{}\n   {}", prompt, warning.dimmed()),
                    false,
                ) {
                    if !quiet {
                        println!(
                            "\n{} Uninstalling dependency: {}...",
                            "::".bold().blue(),
                            dep_str.bold()
                        );
                    }
                } else {
                    if !quiet {
                        println!(
                            "Skipping uninstallation of native dependency: {}",
                            dep.package.yellow()
                        );
                    }
                    continue;
                }
            } else {
                if !quiet {
                    println!(
                        "\n{} Uninstalling zoi dependency: {}...",
                        "::".bold().blue(),
                        dep_str.bold()
                    );
                }
            }

            if let Err(e) = dependencies::uninstall_dependency(dep_str, &move |name| {
                run(name, Some(scope), yes, quiet, dry_run).map(|_| ())
            }) && !quiet
            {
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
        let _ = cleanup_service(&pkg.name, scope);
        fs::remove_dir_all(&package_dir)?;
    }
    if let Err(e) = recorder::remove_package_from_record(manifest)
        && !quiet
    {
        eprintln!(
            "{} Failed to remove package from lockfile: {}",
            "Warning:".yellow(),
            e
        );
    }

    if let Ok(conn) = db::open_connection("local") {
        let _ = db::delete_package(&conn, &pkg.name, None, &pkg.repo, Some(scope));
    }

    if let Ok(true) = telemetry::posthog_capture_event(
        "uninstall",
        pkg,
        env!("CARGO_PKG_VERSION"),
        registry_handle.as_deref().unwrap_or("local"),
        None,
    ) && !quiet
    {
        println!("{} telemetry sent", "Info:".green());
    }

    Ok(manifest.clone())
}

fn find_installed_manifest(
    request: &resolve::PackageRequest,
    scope_override: Option<types::Scope>,
) -> anyhow::Result<(types::InstallManifest, types::Scope)> {
    let scopes = if let Some(scope) = scope_override {
        vec![scope]
    } else {
        vec![
            types::Scope::Project,
            types::Scope::User,
            types::Scope::System,
        ]
    };

    for scope in scopes {
        let mut matches = local::find_installed_manifests_matching(request, scope)?;
        match matches.len() {
            0 => continue,
            1 => return Ok((matches.remove(0), scope)),
            _ => {
                return Err(anyhow!(
                    "Package '{}' is ambiguous in {:?} scope. Use an explicit source like '#handle@repo/name[:sub]@version'.",
                    request.name,
                    scope
                ));
            }
        }
    }

    if scope_override.is_some() {
        Err(anyhow!(
            "Package '{}' is not installed in the specified scope.",
            request.name
        ))
    } else {
        Err(anyhow!(
            "Package '{}' is not installed by Zoi.",
            request.name
        ))
    }
}

fn load_installed_package(
    manifest: &types::InstallManifest,
    yes: bool,
) -> anyhow::Result<(types::Package, PathBuf)> {
    let installed_source_path = local::get_package_source_path(manifest)?;
    if installed_source_path.exists() {
        let path = installed_source_path
            .to_str()
            .ok_or_else(|| anyhow!("Stored package source path contains invalid UTF-8"))?;
        let mut pkg = zoi_lua::parser::parse_lua_package(
            path,
            Some(&manifest.version),
            Some(manifest.scope),
            true,
        )?;
        pkg.repo = manifest.repo.clone();
        pkg.scope = manifest.scope;
        pkg.registry_handle = Some(manifest.registry_handle.clone());
        pkg.sub_package = manifest.sub_package.clone();
        return Ok((pkg, installed_source_path));
    }

    let source = local::installed_manifest_source(manifest);
    let (mut pkg, _, _, pkg_lua_path, _, _, _) =
        resolve::resolve_package_and_version(&source, Some(manifest.scope), true, yes)?;
    pkg.scope = manifest.scope;
    pkg.sub_package = manifest.sub_package.clone();
    Ok((pkg, pkg_lua_path))
}

/// Uninstalls one or more packages from the system.
///
/// This is a complex multi-stage operation:
/// - Dependent Check: Verifies if any other package requires this one
///   (via the `dependents/` directory). Blocks if busy.
/// - Hook Execution: Runs the `pre_remove` hook defined in `.pkg.lua`.
/// - Lua Cleanup: Executes the `uninstall()` function and `zrm` operations.
/// - File Removal: Deletes every file recorded in the package's `InstallManifest`.
/// - Shim/Completion Cleanup: Unlinks binaries and completions if no other
///   package provides them (ref-counting via the database).
///
/// If `recursive` is true, Zoi also attempts to uninstall any dependencies
/// that are no longer needed by any other package.
pub fn run(
    package_name: &str,
    scope_override: Option<types::Scope>,
    yes: bool,
    quiet: bool,
    dry_run: bool,
) -> anyhow::Result<types::InstallManifest> {
    let request = resolve::parse_source_string(package_name)?;
    let (manifest, scope) = find_installed_manifest(&request, scope_override)?;
    let sub_package_to_uninstall = manifest.sub_package.clone();
    let registry_handle = Some(manifest.registry_handle.clone());
    let (pkg, pkg_lua_path) = load_installed_package(&manifest, yes)?;

    if pkg.package_type == types::PackageType::Collection {
        return uninstall_collection(
            &pkg,
            &manifest,
            scope,
            registry_handle.clone(),
            yes,
            quiet,
            dry_run,
        );
    }

    if dry_run {
        return Ok(manifest);
    }

    let handle = manifest.registry_handle.as_str();
    let package_dir = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;
    let version_dir = package_dir.join(&manifest.version);

    let dependents = local::get_dependents(&package_dir)?;
    if !dependents.is_empty() {
        return Err(anyhow::anyhow!(
            "Cannot uninstall '{}' because other packages depend on it:\n  -{}\n\nPlease uninstall these packages first.",
            pkg.name,
            dependents.join("\n  - ")
        ));
    }

    let needs_escalation = scope == types::Scope::System && !core_utils::is_admin();

    if needs_escalation {
        if !quiet {
            println!(
                "{} Escalating to root to remove system package...",
                "::".bold().blue()
            );
        }
        let manifest_json = serde_json::to_string(&manifest)?;
        let mut temp_file = tempfile::NamedTempFile::new()?;
        use std::io::Write;
        temp_file.write_all(manifest_json.as_bytes())?;
        let temp_path = temp_file.path();

        let mut cmd = std::process::Command::new("sudo");
        cmd.arg(std::env::current_exe()?);
        cmd.arg("helper").arg("elevate-uninstall");
        cmd.arg("--manifest-json").arg(temp_path);
        if yes {
            cmd.arg("--yes");
        }

        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to spawn sudo: {}", e))?;
        if !status.success() {
            return Err(anyhow::anyhow!("Escalated uninstallation failed."));
        }
    } else {
        if let Some(hooks) = &pkg.hooks
            && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PreRemove, scope)
        {
            return Err(anyhow::anyhow!("Pre-remove hook failed: {}", e));
        }

        let lua = Lua::new();
        zoi_lua::functions::setup_lua_environment(
            &lua,
            &core_utils::get_platform()?,
            Some(&manifest.version),
            pkg_lua_path.to_str(),
            None,
            None,
            None,
            sub_package_to_uninstall.as_deref(),
            Some(scope),
            None,
            true,
        )
        .map_err(|e| anyhow!(e.to_string()))?;
        let lua_code = fs::read_to_string(pkg_lua_path)?;
        lua.load(&lua_code)
            .exec()
            .map_err(|e| anyhow!(e.to_string()))?;

        if let Ok(uninstall_fn) = lua.globals().get::<mlua::Function>("uninstall") {
            if !quiet {
                println!("Running uninstall() script...");
            }
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
                    let mut path_to_remove: String =
                        op.get("path").map_err(|e| anyhow!(e.to_string()))?;

                    path_to_remove =
                        path_to_remove.replace("${pkgstore}", &version_dir.to_string_lossy());

                    if let Some(home_dir) = core_utils::get_user_home() {
                        path_to_remove =
                            path_to_remove.replace("${usrhome}", &home_dir.to_string_lossy());
                    }
                    path_to_remove = path_to_remove.replace(
                        "${usrroot}",
                        &sysroot::apply_sysroot(PathBuf::from("/")).to_string_lossy(),
                    );

                    let path = std::path::PathBuf::from(path_to_remove);
                    if path.exists() {
                        if !quiet {
                            println!("Removing {}...", path.display());
                        }
                        if path.is_dir() {
                            fs::remove_dir_all(path)?;
                        } else {
                            fs::remove_file(path)?;
                        }
                    }
                }
            }
        }

        if let Some(backup_files) = &manifest.backup {
            if !quiet {
                println!("Saving configuration files...");
            }
            for backup_file_rel in backup_files {
                let backup_src = version_dir.join(backup_file_rel);
                if backup_src.exists() {
                    let backup_dest = version_dir
                        .parent()
                        .ok_or_else(|| anyhow!("version_dir should have a parent (package_dir)"))?
                        .join(format!("{}.zoisave", backup_file_rel));
                    if let Some(p) = backup_dest.parent()
                        && let Err(e) = fs::create_dir_all(p)
                    {
                        if !quiet {
                            eprintln!(
                                "Warning: could not create backup directory {}: {}",
                                p.display(),
                                e
                            );
                        }
                        continue;
                    }
                    if !quiet {
                        println!(
                            "Saving {} to {}",
                            backup_src.display(),
                            backup_dest.display()
                        );
                    }
                    if let Err(e) = fs::rename(&backup_src, &backup_dest)
                        && !quiet
                    {
                        eprintln!("Warning: failed to save {}: {}", backup_src.display(), e);
                    }
                }
            }
        }

        if !quiet {
            println!(
                "Uninstalling '{}'...",
                if let Some(sub) = &manifest.sub_package {
                    format!("{}:{}", pkg.name, sub)
                } else {
                    pkg.name.clone()
                }
                .bold()
            );
        }

        if let Some(bins) = &manifest.bins {
            let bin_root = get_bin_root(scope)?;
            for bin in bins {
                let symlink_path = bin_root.join(bin);
                if symlink_path.is_symlink() || symlink_path.exists() {
                    let other_providers = db::find_provides("local", bin)?;
                    let still_provided = other_providers.iter().any(|(p, _)| {
                        p.name != pkg.name || (p.sub_package != manifest.sub_package)
                    });

                    if !still_provided {
                        if !quiet {
                            println!(
                                "Removing shim for {} from {}...",
                                bin.cyan(),
                                symlink_path.display()
                            );
                        }
                        fs::remove_file(&symlink_path)?;
                    } else {
                        if !quiet {
                            println!(
                                "Keeping shim for {} as it is still provided by other packages.",
                                bin.cyan()
                            );
                        }
                    }
                }
            }
        } else if manifest.sub_package.is_none() {
            let bin = &pkg.name;
            let symlink_path = get_bin_root(scope)?.join(bin);
            if symlink_path.is_symlink() || symlink_path.exists() {
                let other_providers = db::find_provides("local", bin)?;
                let still_provided = other_providers
                    .iter()
                    .any(|(p, _)| p.name != pkg.name || (p.sub_package != manifest.sub_package));

                if !still_provided {
                    if !quiet {
                        println!(
                            "Removing shim for {} from {}...",
                            bin.cyan(),
                            symlink_path.display()
                        );
                    }
                    fs::remove_file(symlink_path)?;
                }
            }
        }

        if let Some(completions) = &manifest.completions {
            for completion in completions {
                let completions_root = get_completions_root(scope, &completion.shell)?;
                let pkg_dir = completions_root.join(&pkg.name);
                let symlink_path = pkg_dir.join(&completion.filename);
                if symlink_path.is_symlink() || symlink_path.exists() {
                    let other_providers = db::find_provides("local", &completion.filename)?;
                    let still_provided = other_providers.iter().any(|(p, _)| {
                        p.name != pkg.name || (p.sub_package != manifest.sub_package)
                    });

                    if !still_provided {
                        if !quiet {
                            println!(
                                "Removing {} completion for {} from {}...",
                                completion.shell.cyan(),
                                completion.filename.cyan(),
                                symlink_path.display()
                            );
                        }
                        fs::remove_file(&symlink_path)?;
                    } else if !quiet {
                        println!(
                            "Keeping {} completion for {} as it is still provided by other packages.",
                            completion.shell.cyan(),
                            completion.filename.cyan()
                        );
                    }
                }
            }

            let shells: std::collections::HashSet<String> =
                completions.iter().map(|c| c.shell.clone()).collect();
            for shell_name in shells {
                let pkg_dir = get_completions_root(scope, &shell_name)?.join(&pkg.name);
                if pkg_dir.exists()
                    && fs::read_dir(&pkg_dir)
                        .map(|mut e| e.next().is_none())
                        .unwrap_or(false)
                {
                    let _ = fs::remove_dir(&pkg_dir);
                }
            }
        }

        let pkg_id_opt = if let Ok(conn) = db::open_connection("local") {
            db::get_package_id(
                &conn,
                &pkg.name,
                manifest.sub_package.as_deref(),
                &pkg.repo,
                handle,
            )
            .ok()
        } else {
            None
        };

        for file_path_str in &manifest.installed_files {
            let expanded = core_utils::expand_placeholders(file_path_str, &version_dir, scope)?;
            let file_path = PathBuf::from(&expanded);

            if let Some(pkg_id) = pkg_id_opt
                && let Ok(conn) = db::open_connection("local")
                && let Ok(true) = db::has_other_owners(&conn, file_path_str, pkg_id)
            {
                if !quiet {
                    println!(
                        "Keeping {} as it is still owned by other packages.",
                        file_path_str.dimmed()
                    );
                }
                continue;
            }

            if file_path.exists() {
                if file_path.is_dir() {
                    // Only remove if empty to be safe
                    if fs::read_dir(&file_path)
                        .map(|mut e| e.next().is_none())
                        .unwrap_or(false)
                    {
                        let _ = fs::remove_dir_all(&file_path);
                    }
                } else {
                    let _ = fs::remove_file(&file_path);
                }
            }
        }

        let manifest_filename = if let Some(sub) = &manifest.sub_package {
            format!("manifest-{}.yaml", sub)
        } else {
            "manifest.yaml".to_string()
        };
        let manifest_path = version_dir.join(manifest_filename);
        if manifest_path.exists() {
            fs::remove_file(manifest_path)?;
        }

        if version_dir.exists() {
            let mut has_other_manifests = false;
            if let Ok(entries) = fs::read_dir(&version_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("manifest") && name.ends_with(".yaml") {
                        has_other_manifests = true;
                        break;
                    }
                }
            }
            if !has_other_manifests {
                if !quiet {
                    println!(
                        "Removing empty version directory: {}",
                        version_dir.display()
                    );
                }
                fs::remove_dir_all(&version_dir)?;
            }
        }

        if package_dir.exists() {
            let _ = cleanup_service(&pkg.name, scope);
            let mut has_other_versions = false;
            if let Ok(entries) = fs::read_dir(&package_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name != "latest" && name != "dependents" {
                        has_other_versions = true;
                        break;
                    }
                }
            }
            if !has_other_versions {
                if !quiet {
                    println!("Removing package store: {}", package_dir.display());
                }
                fs::remove_dir_all(&package_dir)?;
            }
        }

        let parent_id = format!(
            "#{}@{}/{}@{}",
            manifest.registry_handle, manifest.repo, manifest.name, manifest.version
        );
        for dep_str in &manifest.installed_dependencies {
            if let Ok(dep) = dependencies::parse_dependency_string(dep_str)
                && dep.manager == "zoi"
            {
                let dep_req = resolve::parse_source_string(dep.package)?;
                let dep_matches = local::find_installed_manifests_matching(&dep_req, scope)?;
                if dep_matches.len() == 1 {
                    let dep_manifest = &dep_matches[0];
                    match local::get_package_dir(
                        dep_manifest.scope,
                        &dep_manifest.registry_handle,
                        &dep_manifest.repo,
                        &dep_manifest.name,
                    ) {
                        Ok(dep_pkg_dir) => {
                            if let Err(e) = local::remove_dependent(&dep_pkg_dir, &parent_id)
                                && !quiet
                            {
                                eprintln!(
                                    "Warning: failed to remove dependent link for {}: {}",
                                    dep.package, e
                                );
                            }
                        }
                        Err(e) => {
                            if !quiet {
                                eprintln!(
                                    "Warning: failed to get package dir for {}: {}",
                                    dep.package, e
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(hooks) = &pkg.hooks
            && let Err(e) = hooks::run_hooks(hooks, hooks::HookType::PostRemove, scope)
            && !quiet
        {
            eprintln!("{} post-remove hook failed: {}", "Warning:".yellow(), e);
        }
    }

    if let Err(e) = recorder::remove_package_from_record(&manifest)
        && !quiet
    {
        eprintln!(
            "{} Failed to remove package from lockfile: {}",
            "Warning:".yellow(),
            e
        );
    }

    if let Ok(conn) = db::open_connection("local") {
        let _ = db::delete_package(
            &conn,
            &pkg.name,
            sub_package_to_uninstall.as_deref(),
            &pkg.repo,
            Some(scope),
        );
    }

    if !quiet {
        println!("Removed manifest for '{}'.", pkg.name);
    }

    if let Ok(true) = telemetry::posthog_capture_event(
        "uninstall",
        &pkg,
        env!("CARGO_PKG_VERSION"),
        &manifest.registry_handle,
        None,
    ) && !quiet
    {
        println!("{} telemetry sent", "Info:".green());
    }

    Ok(manifest)
}
