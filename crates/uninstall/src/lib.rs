//! Uninstallation logic for Zoi packages.
//!
//! This crate handles the safe removal of packages, including cleaning up
//! binaries, completion scripts, service units, and dependency management.

/// Logic for automatically removing unused dependencies.
pub mod autoremove;

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::anyhow;
use colored::Colorize;
use mlua::Lua;
use zoi_core::{recorder, sysroot, types, utils as core_utils};
use zoi_db as db;
use zoi_deps as dependencies;
use zoi_hooks as hooks;
use zoi_resolver::{local, resolve};
use zoi_telemetry as telemetry;

/// Gets the root directory for binaries based on the installation scope.
fn get_bin_root(scope: types::Scope) -> anyhow::Result<PathBuf> {
    match scope {
        types::Scope::User => core_utils::get_user_bin_dir(),
        types::Scope::System => Ok(core_utils::get_system_bin_dir()),
        types::Scope::Project => {
            let current_dir = std::env::current_dir()?;
            Ok(current_dir.join(".zoi").join("pkgs").join("bin"))
        }
    }
}

/// Gets the root directory for shell completions based on the scope and shell
/// type.
fn get_completions_root(
    scope: types::Scope,
    shell: &str
) -> anyhow::Result<PathBuf> {
    match scope {
        types::Scope::User => core_utils::get_user_completions_dir(shell),
        types::Scope::System => {
            if cfg!(target_os = "windows") {
                Ok(sysroot::apply_sysroot(PathBuf::from(format!(
                    "C:\\ProgramData\\zoi\\pkgs\\shell\\{shell}",
                ))))
            } else {
                let base = match shell {
                    "bash" => "/usr/share/bash-completion/completions",
                    "zsh" => "/usr/share/zsh/site-functions",
                    "fish" => "/usr/share/fish/vendor_completions.d",
                    "elvish" => "/usr/share/elvish/lib",
                    _ => "/usr/local/share/zoi/completions"
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

/// Cleans up service unit files or Windows services associated with a package.
fn cleanup_service(
    package_name: &str,
    scope: types::Scope
) -> anyhow::Result<()> {
    let service_name = format!("zoi-{package_name}");
    let is_user = scope != types::Scope::System;

    match std::env::consts::OS {
        "linux" => {
            let unit_path = if is_user {
                let home = core_utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"))?;
                sysroot::apply_sysroot(
                    home.join(".config/systemd/user")
                        .join(format!("{service_name}.service"))
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/etc/systemd/system/{service_name}.service",
                )))
            };
            if unit_path.exists() {
                println!("Removing service unit file: {}", unit_path.display());
                fs::remove_file(&unit_path).map_err(|e| {
                    anyhow!(
                        "Failed to remove unit file: {}: {}",
                        unit_path.display(),
                        e
                    )
                })?;
                if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err() {
                    let mut cmd = std::process::Command::new("systemctl");
                    if is_user {
                        cmd.arg("--user");
                    }
                    cmd.arg("daemon-reload").status().map_err(|e| {
                        anyhow!("Failed to run systemctl daemon-reload: {e}")
                    })?;
                }
            }
        }
        "macos" => {
            let plist_path = if is_user {
                let home = core_utils::get_user_home()
                    .ok_or_else(|| anyhow!("Could not find home directory"))?;
                sysroot::apply_sysroot(
                    home.join("Library/LaunchAgents")
                        .join(format!("{service_name}.plist"))
                )
            } else {
                sysroot::apply_sysroot(PathBuf::from(format!(
                    "/Library/LaunchDaemons/{service_name}.plist",
                )))
            };
            if plist_path.exists() {
                println!(
                    "Removing service plist file: {}",
                    plist_path.display()
                );
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
                    .map_err(|e| anyhow!("Failed to run sc query: {e}"))?;
                output.status.success()
            };
            if std::env::var("ZOI_TEST_SKIP_SERVICE_COMMANDS").is_err()
                && exists
            {
                println!("Removing Windows service: {service_name}");
                std::process::Command::new("sc")
                    .arg("delete")
                    .arg(&service_name)
                    .status()
                    .map_err(|e| anyhow!("Failed to run sc delete: {e}"))?;
            }
        }
        _ => {}
    }

    Ok(())
}

/// Uninstalls a collection and its associated dependencies.
fn uninstall_collection(
    pkg: &types::Package,
    manifest: &types::InstallManifest,
    scope: types::Scope,
    registry_handle: Option<&str>,
    yes: bool,
    quiet: bool,
    dry_run: bool
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

            if dep.manager == "zoi" {
                if !quiet {
                    println!(
                        "\n{} Uninstalling zoi dependency: {}...",
                        "::".bold().blue(),
                        dep_str.bold()
                    );
                }
            } else {
                let prompt = format!(
                    "Uninstall native dependency '{}' ({})?",
                    dep.package.cyan(),
                    dep.manager.yellow()
                );
                let warning = "Warning: Zoi cannot track if other non-Zoi \
                               applications depend on this package.";

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
                    false
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
            }

            if let Err(e) =
                dependencies::uninstall_dependency(dep_str, &move |name| {
                    run(name, Some(scope), yes, quiet, dry_run).map(|_| ())
                })
                && !quiet
            {
                eprintln!(
                    "Warning: Could not uninstall dependency '{dep_str}': {e}"
                );
            }
        }
    }

    let handle = registry_handle.unwrap_or("local");
    let package_dir =
        local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;
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
        let _ =
            db::delete_package(&conn, &pkg.name, None, &pkg.repo, Some(scope));
    }

    if let Ok(true) = telemetry::posthog_capture_event(
        "uninstall",
        pkg,
        env!("CARGO_PKG_VERSION"),
        registry_handle.unwrap_or("local"),
        None
    ) && !quiet
    {
        println!("{} telemetry sent", "Info:".green());
    }

    Ok(manifest.clone())
}

/// Finds the active installed manifest, ignoring any version pin in the
/// request.
///
/// Installed manifests are only indexed under `latest/`, so a request like
/// `package@1.2.0` must first resolve the package identity (name, repo,
/// handle, sub-package, scope) without the version, then decide which stored
/// version the pin refers to.
fn find_active_manifest(
    request: &resolve::PackageRequest,
    scope_override: Option<types::Scope>
) -> anyhow::Result<(types::InstallManifest, types::Scope)> {
    let mut unversioned = resolve::PackageRequest {
        handle: request.handle.clone(),
        repo: request.repo.clone(),
        name: request.name.clone(),
        sub_package: request.sub_package.clone(),
        version_spec: None
    };
    let _ = &mut unversioned;
    find_installed_manifest(&unversioned, scope_override)
}

/// Lists every stored version of a package that has a manifest for the given
/// sub-package.
///
/// Returns `(version, manifest)` pairs sorted by version ascending (semver
/// when parseable, lexical fallback).
fn list_stored_version_manifests(
    package_dir: &std::path::Path,
    sub_package: Option<&str>
) -> Vec<(String, types::InstallManifest)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(package_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "latest" || name == "dependents" {
            continue;
        }
        let version_dir = entry.path();
        if !version_dir.is_dir() {
            continue;
        }
        let manifest_path =
            local::find_store_manifest(&version_dir, sub_package);
        let Some(manifest_path) = manifest_path else {
            continue;
        };
        if let Ok(manifest) = local::read_store_manifest(&manifest_path)
            && manifest.sub_package.as_deref() == sub_package
        {
            out.push((name, manifest));
        }
    }
    out.sort_by(|a, b| compare_versions(&a.0, &b.0));
    out
}

/// Searches every store directory for a specific stored version.
///
/// Used when the `latest` link is missing but a versioned uninstall was
/// requested. Returns the stored manifest and its scope.
///
/// This is also used by the CLI to resolve `zoi uninstall package@1.2.0`
/// when `1.2.0` is no longer the active version.
pub fn find_stored_version_anywhere(
    request: &resolve::PackageRequest,
    scope_override: Option<types::Scope>
) -> Option<(types::InstallManifest, types::Scope)> {
    let version = request.version_spec.as_deref()?;
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
        let Ok(store_root) = local::get_store_base_dir(scope) else {
            continue;
        };
        if !store_root.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&store_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let package_dir = entry.path();
            if !package_dir.is_dir() {
                continue;
            }
            for (stored_version, manifest) in list_stored_version_manifests(
                &package_dir,
                request.sub_package.as_deref()
            ) {
                if stored_version != version {
                    continue;
                }
                if !manifest.name.eq_ignore_ascii_case(&request.name) {
                    continue;
                }
                if request.handle.as_ref().is_some_and(|handle| {
                    !manifest.registry_handle.eq_ignore_ascii_case(handle)
                }) {
                    continue;
                }
                if request.repo.as_ref().is_some_and(|repo| {
                    !manifest.repo.eq_ignore_ascii_case(repo)
                }) {
                    continue;
                }
                return Some((manifest, scope));
            }
        }
    }
    None
}

/// Compares two version strings with semver when possible.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (
        semver::Version::parse(a.trim_start_matches('v')),
        semver::Version::parse(b.trim_start_matches('v'))
    ) {
        (Ok(va), Ok(vb)) => va.cmp(&vb),
        _ => a.cmp(b)
    }
}

/// Removes a non-active stored version directory without touching the active
/// install (shims, database, lockfile).
fn remove_stored_version_dir(
    package_dir: &std::path::Path,
    version: &str,
    sub_package: Option<&str>,
    quiet: bool
) -> anyhow::Result<types::InstallManifest> {
    let version_dir = package_dir.join(version);
    let manifest_path = local::find_store_manifest(&version_dir, sub_package)
        .ok_or_else(|| {
        anyhow!(
            "Version '{version}' has no install manifest in {}",
            version_dir.display()
        )
    })?;
    let manifest = local::read_store_manifest(&manifest_path)?;
    if manifest_path.exists() {
        fs::remove_file(&manifest_path)?;
    }
    let legacy_path = version_dir.join(format!(
        "manifest{}.yaml",
        sub_package.map_or_else(String::new, |sub| format!("-{sub}"))
    ));
    if legacy_path != manifest_path && legacy_path.exists() {
        fs::remove_file(&legacy_path)?;
    }
    // Remove the stored source snapshot for this version as well.
    let source_path = version_dir.join("package.pkg.lua");
    if source_path.exists() {
        let _ = fs::remove_file(&source_path);
    }
    if version_dir.exists() {
        let is_empty =
            fs::read_dir(&version_dir).is_ok_and(|mut e| e.next().is_none());
        if is_empty {
            fs::remove_dir_all(&version_dir)?;
        } else if !quiet {
            println!(
                "Keeping non-empty version directory: {}",
                version_dir.display()
            );
        }
    }
    Ok(manifest)
}

/// Loads the richest package definition available for a stored manifest.
///
/// Prefers the `package.pkg.lua` snapshot stored next to the manifest so the
/// database and lockfile keep full metadata (bins, description, tags);
/// falls back to the manifest-derived blueprint.
fn load_stored_package(
    version_dir: &std::path::Path,
    manifest: &types::InstallManifest
) -> types::Package {
    let stored_source = version_dir.join("package.pkg.lua");
    if stored_source.exists()
        && let Some(path_str) = stored_source.to_str()
        && let Ok(mut pkg) = zoi_lua::parser::parse_lua_package(
            path_str,
            Some(&manifest.version),
            Some(manifest.scope),
            true
        )
    {
        pkg.repo.clone_from(&manifest.repo);
        pkg.scope = manifest.scope;
        pkg.registry_handle = Some(manifest.registry_handle.clone());
        pkg.sub_package.clone_from(&manifest.sub_package);
        return pkg;
    }
    let mut pkg = manifest.clone().into_package();
    pkg.description.clone_from(&manifest.description);
    pkg.registry_handle = Some(manifest.registry_handle.clone());
    pkg
}

/// Re-creates binary shims for a stored manifest being re-activated.
fn restore_version_shims(
    manifest: &types::InstallManifest,
    scope: types::Scope,
    quiet: bool
) {
    let bins: Vec<String> = manifest.bins.clone().unwrap_or_else(|| {
        if manifest.sub_package.is_none() {
            vec![manifest.name.clone()]
        } else {
            Vec::new()
        }
    });
    if bins.is_empty() {
        return;
    }
    let Ok(bin_root) = get_bin_root(scope) else {
        return;
    };
    if let Err(e) = fs::create_dir_all(&bin_root) {
        if !quiet {
            eprintln!(
                "{} could not create bin dir {}: {}",
                "Warning:".yellow(),
                bin_root.display(),
                e
            );
        }
        return;
    }
    let Ok(zoi_exe) = std::env::current_exe() else {
        return;
    };
    for bin in &bins {
        let link_path = bin_root.join(bin);
        if let Err(e) = core_utils::symlink_file(&zoi_exe, &link_path) {
            if !quiet {
                eprintln!(
                    "{} failed to restore shim for {bin}: {}",
                    "Warning:".yellow(),
                    e
                );
            }
        } else if !quiet {
            println!("Restored shim for {}...", bin.cyan());
        }
    }
}

/// Re-creates shell completion symlinks for a stored manifest.
fn restore_version_completions(
    manifest: &types::InstallManifest,
    version_dir: &std::path::Path,
    scope: types::Scope,
    quiet: bool
) {
    let Some(completions) = &manifest.completions else {
        return;
    };
    for completion in completions {
        // Current archives stage completions under `<version>/shell/<shell>/`,
        // older ones under `<version>/data/shell/<shell>/`; try both.
        let candidates = [
            version_dir
                .join("shell")
                .join(&completion.shell)
                .join(&completion.filename),
            version_dir
                .join("data")
                .join("shell")
                .join(&completion.shell)
                .join(&completion.filename)
        ];
        let Some(store_path) = candidates.iter().find(|p| p.exists()) else {
            if !quiet {
                eprintln!(
                    "{} completion source for {} not found in store, skipping.",
                    "Warning:".yellow(),
                    completion.filename.cyan()
                );
            }
            continue;
        };
        let Ok(completions_root) =
            get_completions_root(scope, &completion.shell)
        else {
            continue;
        };
        let pkg_dir = completions_root.join(&manifest.name);
        let link_path = pkg_dir.join(&completion.filename);
        if let Some(parent) = link_path.parent()
            && let Err(e) = fs::create_dir_all(parent)
            && !quiet
        {
            eprintln!(
                "{} could not create completions dir {}: {}",
                "Warning:".yellow(),
                parent.display(),
                e
            );
        }
        #[cfg(unix)]
        let mut link_result =
            std::os::unix::fs::symlink(store_path, &link_path)
                .map_err(|e| anyhow!(e.to_string()));
        #[cfg(windows)]
        let mut link_result =
            std::os::windows::fs::symlink_file(store_path, &link_path)
                .map_err(|e| anyhow!(e.to_string()));
        // `symlink` fails if a link already exists; replace and retry once.
        if link_result.is_err() {
            let _ = fs::remove_file(&link_path);
            #[cfg(unix)]
            {
                link_result =
                    std::os::unix::fs::symlink(store_path, &link_path)
                        .map_err(|e| anyhow!(e.to_string()));
            }
            #[cfg(windows)]
            {
                link_result =
                    std::os::windows::fs::symlink_file(store_path, &link_path)
                        .map_err(|e| anyhow!(e.to_string()));
            }
        }
        if let Err(e) = link_result
            && !quiet
        {
            eprintln!(
                "{} failed to restore completion {}: {}",
                "Warning:".yellow(),
                completion.filename.cyan(),
                e
            );
        }
    }
}

/// Activates a previously stored version after its newer version was
/// uninstalled.
///
/// This mirrors `rollback`: it only uses what is already in the store and
/// never downloads or reinstalls from a registry. It re-points `latest`,
/// restores shims and completions, and updates the database and lockfile so
/// the rolled-back version becomes the recorded install.
fn activate_stored_version(
    package_dir: &std::path::Path,
    scope: types::Scope,
    version: &str,
    manifest: &types::InstallManifest,
    quiet: bool
) -> anyhow::Result<()> {
    let version_dir = package_dir.join(version);
    // Re-point `latest` at the previous version (same helper installs use).
    if let Err(e) =
        core_utils::symlink_dir(&version_dir, &package_dir.join("latest"))
    {
        return Err(anyhow!(
            "Failed to point 'latest' at version '{version}': {e}"
        ));
    }
    // Persist through the canonical writer as well so JSON/legacy twins and
    // symlink handling stay consistent with installs.
    if let Err(e) = local::write_manifest(manifest)
        && !quiet
    {
        eprintln!(
            "{} failed to rewrite manifest for version '{version}': {}",
            "Warning:".yellow(),
            e
        );
    }
    restore_version_shims(manifest, scope, quiet);
    restore_version_completions(manifest, &version_dir, scope, quiet);

    let pkg = load_stored_package(&version_dir, manifest);
    if let Ok(conn) = db::open_connection("local")
        && let Ok(pkg_id) = db::update_package(
            &conn,
            &pkg,
            &manifest.registry_handle,
            Some(scope),
            manifest.sub_package.as_deref(),
            Some(&manifest.reason)
        )
    {
        let _ = db::clear_package_files(&conn, pkg_id);
        let _ =
            db::index_package_files(&conn, pkg_id, &manifest.installed_files);
    }
    if let Err(e) = recorder::record_package(
        &pkg,
        &manifest.reason,
        &manifest.installed_dependencies,
        &manifest.registry_handle,
        &manifest.repo_type,
        &manifest.chosen_options,
        &manifest.chosen_optionals,
        manifest.sub_package.as_deref()
    ) && !quiet
    {
        eprintln!(
            "{} failed to record rolled-back version '{version}': {}",
            "Warning:".yellow(),
            e
        );
    }
    if !quiet {
        println!(
            "{} Activated previous version {} from the store.",
            "::".bold().green(),
            version.green()
        );
    }
    Ok(())
}

/// Finds an installed manifest matching the given package request.
fn find_installed_manifest(
    request: &resolve::PackageRequest,
    scope_override: Option<types::Scope>
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
        let mut matches =
            local::find_installed_manifests_matching(request, scope)?;
        match matches.len() {
            0 => {}
            1 => return Ok((matches.remove(0), scope)),
            _ => {
                return Err(anyhow!(
                    "Package '{}' is ambiguous in {:?} scope. Use an explicit \
                     source like '#handle@repo/name[:sub]@version'.",
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

/// Loads an installed package definition and its Lua source path from a
/// manifest.
fn load_installed_package(
    manifest: &types::InstallManifest,
    yes: bool
) -> anyhow::Result<(types::Package, PathBuf)> {
    let installed_source_path = local::get_package_source_path(manifest)?;
    if installed_source_path.exists() {
        let path = installed_source_path.to_str().ok_or_else(|| {
            anyhow!("Stored package source path contains invalid UTF-8")
        })?;
        let mut pkg = zoi_lua::parser::parse_lua_package(
            path,
            Some(&manifest.version),
            Some(manifest.scope),
            true
        )?;
        pkg.repo.clone_from(&manifest.repo);
        pkg.scope = manifest.scope;
        pkg.registry_handle = Some(manifest.registry_handle.clone());
        pkg.sub_package.clone_from(&manifest.sub_package);
        return Ok((pkg, installed_source_path));
    }

    let source = local::installed_manifest_source(manifest);
    let (mut pkg, _, _, pkg_lua_path, _, _, _) =
        resolve::resolve_package_and_version(
            &source,
            Some(manifest.scope),
            true,
            yes
        )?;
    pkg.scope = manifest.scope;
    pkg.sub_package.clone_from(&manifest.sub_package);
    Ok((pkg, pkg_lua_path))
}

/// Uninstalls one or more packages from the system.
///
/// This is a complex multi-stage operation:
/// - Dependent Check: Verifies if any other package requires this one (via the
///   `dependents/` directory). Blocks if busy.
/// - Hook Execution: Runs the `pre_remove` hook defined in `.pkg.lua`.
/// - Lua Cleanup: Executes the `uninstall()` function and `zrm` operations.
/// - File Removal: Deletes every file recorded in the package's
///   `InstallManifest`.
/// - Shim/Completion Cleanup: Unlinks binaries and completions if no other
///   package provides them (ref-counting via the database).
///
/// If `recursive` is true, Zoi also attempts to uninstall any dependencies
/// that are no longer needed by any other package.
///
/// # Errors
///
/// Returns an error if:
/// - The package is not found or is ambiguous.
/// - The package has dependents that must be uninstalled first.
/// - Hook execution fails.
/// - File system operations (removal, backup) fail.
/// - Escalation to root fails.
///
/// # Panics
///
/// Panics if internal dependency consistency checks fail.
pub fn run(
    package_name: &str,
    scope_override: Option<types::Scope>,
    yes: bool,
    quiet: bool,
    dry_run: bool
) -> anyhow::Result<types::InstallManifest> {
    let request = resolve::parse_source_string(package_name)?;
    // Resolve the package identity through the active (`latest`) manifest,
    // ignoring any version pin. The pin is handled below against the store
    // so `zoi uninstall package@1.2.0` works even when 1.2.0 is not the
    // active version.
    let (manifest, scope) = match find_active_manifest(&request, scope_override)
    {
        Ok(found) => found,
        Err(active_err) => {
            // Fallback for versioned requests when `latest` is missing or
            // broken but the version still exists in the store.
            if let Some(version) = request.version_spec.as_deref() {
                if let Some(found) =
                    find_stored_version_anywhere(&request, scope_override)
                {
                    found
                } else {
                    return Err(anyhow!(
                        "Version '{version}' of package '{}' is not \
                         installed. {}",
                        request.name,
                        active_err
                    ));
                }
            } else {
                return Err(active_err);
            }
        }
    };
    // A version pin that does not point at the active version means "remove
    // just this stored version" - shims, database and lockfile stay on the
    // active install.
    if let Some(version) = request.version_spec.clone()
        && version != manifest.version
    {
        let handle = manifest.registry_handle.as_str();
        let package_dir = local::get_package_dir(
            scope,
            handle,
            &manifest.repo,
            &manifest.name
        )?;
        let stored = list_stored_version_manifests(
            &package_dir,
            manifest.sub_package.as_deref()
        );
        if stored.iter().any(|(v, _)| v == &version) {
            if dry_run {
                return Ok(manifest);
            }
            let dependents = local::get_dependents(&package_dir)?;
            if !dependents.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot uninstall '{}' because other packages depend on \
                     it:\n  -{}\n\nPlease uninstall these packages first.",
                    manifest.name,
                    dependents.join("\n  - ")
                ));
            }
            let removed = remove_stored_version_dir(
                &package_dir,
                &version,
                manifest.sub_package.as_deref(),
                quiet
            )?;
            if !quiet {
                println!(
                    "Removed stored version {} of '{}'. Active version {} is \
                     unchanged.",
                    version.cyan(),
                    manifest.name.bold(),
                    manifest.version.green()
                );
            }
            return Ok(removed);
        }
        let available: Vec<String> =
            stored.iter().map(|(v, _)| v.clone()).collect();
        if available.is_empty() {
            return Err(anyhow!(
                "Version '{version}' of package '{}' is not installed.",
                request.name
            ));
        }
        return Err(anyhow!(
            "Version '{version}' of package '{}' is not installed. Stored \
             versions: {}.",
            request.name,
            available.join(", ")
        ));
    }
    let sub_package_to_uninstall = manifest.sub_package.clone();
    let registry_handle = Some(manifest.registry_handle.clone());
    let (pkg, pkg_lua_path) = load_installed_package(&manifest, yes)?;

    if pkg.package_type == types::PackageType::Collection {
        return uninstall_collection(
            &pkg,
            &manifest,
            scope,
            registry_handle.as_deref(),
            yes,
            quiet,
            dry_run
        );
    }

    if dry_run {
        return Ok(manifest);
    }

    let handle = manifest.registry_handle.as_str();
    let package_dir =
        local::get_package_dir(scope, handle, &pkg.repo, &pkg.name)?;
    let version_dir = package_dir.join(&manifest.version);

    let dependents = local::get_dependents(&package_dir)?;
    if !dependents.is_empty() {
        return Err(anyhow::anyhow!(
            "Cannot uninstall '{}' because other packages depend on it:\n  \
             -{}\n\nPlease uninstall these packages first.",
            pkg.name,
            dependents.join("\n  - ")
        ));
    }

    let needs_escalation =
        scope == types::Scope::System && !core_utils::is_admin();

    if needs_escalation {
        let escalator =
            core_utils::get_privilege_escalator().ok_or_else(|| {
                anyhow!(
                    "Root privileges required to remove system package, but \
                     neither 'sudo' nor 'doas' was found."
                )
            })?;

        if !quiet {
            println!(
                "{} Escalating to root via {} to remove system package...",
                "::".bold().blue(),
                escalator
            );
        }
        let manifest_json = serde_json::to_string(&manifest)?;
        let mut temp_file = tempfile::NamedTempFile::new()?;
        temp_file.write_all(manifest_json.as_bytes())?;
        let temp_path = temp_file.path();

        let mut cmd = std::process::Command::new(escalator);
        cmd.arg(std::env::current_exe()?);
        cmd.arg("helper").arg("elevate-uninstall");
        cmd.arg("--manifest-json").arg(temp_path);
        if yes {
            cmd.arg("--yes");
        }

        let status = cmd.status().map_err(|e| {
            anyhow::anyhow!("Failed to spawn privilege escalator: {e}")
        })?;
        if !status.success() {
            return Err(anyhow::anyhow!("Escalated uninstallation failed."));
        }
    } else {
        if let Some(hooks) = &pkg.hooks
            && let Err(e) =
                hooks::run_hooks(hooks, hooks::HookType::PreRemove, scope)
        {
            return Err(anyhow::anyhow!("Pre-remove hook failed: {e}"));
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
            true
        )
        .map_err(|e| anyhow!(e.to_string()))?;
        let lua_code = fs::read_to_string(pkg_lua_path)?;
        lua.load(&lua_code)
            .exec()
            .map_err(|e| anyhow!(e.to_string()))?;

        if let Ok(uninstall_fn) =
            lua.globals().get::<mlua::Function>("uninstall")
        {
            if !quiet {
                println!("Running uninstall() script...");
            }
            uninstall_fn
                .call::<()>(())
                .map_err(|e| anyhow!(e.to_string()))?;
        }

        if let Ok(uninstall_ops) =
            lua.globals().get::<mlua::Table>("__ZoiUninstallOperations")
        {
            for op in uninstall_ops.sequence_values::<mlua::Table>() {
                let op = op.map_err(|e| anyhow!(e.to_string()))?;
                if let Ok(op_type) = op.get::<String>("op")
                    && op_type == "zrm"
                {
                    let mut path_to_remove: String =
                        op.get("path").map_err(|e| anyhow!(e.to_string()))?;

                    path_to_remove = path_to_remove
                        .replace("${pkgstore}", &version_dir.to_string_lossy());

                    if let Some(home_dir) = core_utils::get_user_home() {
                        path_to_remove = path_to_remove
                            .replace("${usrhome}", &home_dir.to_string_lossy());
                    }
                    path_to_remove = path_to_remove.replace(
                        "${usrroot}",
                        &sysroot::apply_sysroot(PathBuf::from("/"))
                            .to_string_lossy()
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
                let expanded_path = zoi_core::utils::expand_placeholders(
                    backup_file_rel,
                    &version_dir,
                    manifest.scope
                )?;
                let backup_src = PathBuf::from(expanded_path);

                if backup_src.exists() {
                    let backup_filename = backup_src
                        .file_name()
                        .ok_or_else(|| anyhow!("Invalid backup source name"))?
                        .to_string_lossy();
                    let backup_dest = version_dir
                        .parent()
                        .ok_or_else(|| {
                            anyhow!(
                                "version_dir should have a parent \
                                 (package_dir)"
                            )
                        })?
                        .join(format!("{backup_filename}.zoisave"));

                    if let Some(p) = backup_dest.parent()
                        && let Err(e) = fs::create_dir_all(p)
                    {
                        if !quiet {
                            eprintln!(
                                "Warning: could not create backup directory \
                                 {}: {}",
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
                    // Use copy + remove for potential cross-device moves
                    if let Err(e) = fs::copy(&backup_src, &backup_dest) {
                        if !quiet {
                            eprintln!(
                                "Warning: failed to copy backup {}: {}",
                                backup_src.display(),
                                e
                            );
                        }
                    } else {
                        let _ = fs::remove_file(&backup_src);
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
                    let still_provided =
                        other_providers.iter().any(|(p, _)| {
                            p.name != pkg.name
                                || (p.sub_package != manifest.sub_package)
                        });

                    if still_provided {
                        if !quiet {
                            println!(
                                "Keeping shim for {} as it is still provided \
                                 by other packages.",
                                bin.cyan()
                            );
                        }
                    } else {
                        if !quiet {
                            println!(
                                "Removing shim for {} from {}...",
                                bin.cyan(),
                                symlink_path.display()
                            );
                        }
                        fs::remove_file(&symlink_path)?;
                    }
                }
            }
        } else if manifest.sub_package.is_none() {
            let bin = &pkg.name;
            let symlink_path = get_bin_root(scope)?.join(bin);
            if symlink_path.is_symlink() || symlink_path.exists() {
                let other_providers = db::find_provides("local", bin)?;
                let still_provided = other_providers.iter().any(|(p, _)| {
                    p.name != pkg.name
                        || (p.sub_package != manifest.sub_package)
                });

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
                let completions_root =
                    get_completions_root(scope, &completion.shell)?;
                let pkg_dir = completions_root.join(&pkg.name);
                let symlink_path = pkg_dir.join(&completion.filename);
                if symlink_path.is_symlink() || symlink_path.exists() {
                    let other_providers =
                        db::find_provides("local", &completion.filename)?;
                    let still_provided =
                        other_providers.iter().any(|(p, _)| {
                            p.name != pkg.name
                                || (p.sub_package != manifest.sub_package)
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
                            "Keeping {} completion for {} as it is still \
                             provided by other packages.",
                            completion.shell.cyan(),
                            completion.filename.cyan()
                        );
                    }
                }
            }

            let shells: std::collections::HashSet<String> =
                completions.iter().map(|c| c.shell.clone()).collect();
            for shell_name in shells {
                let pkg_dir =
                    get_completions_root(scope, &shell_name)?.join(&pkg.name);
                if pkg_dir.exists()
                    && fs::read_dir(&pkg_dir)
                        .is_ok_and(|mut e| e.next().is_none())
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
                handle
            )
            .ok()
        } else {
            None
        };

        for file_path_str in &manifest.installed_files {
            let expanded = core_utils::expand_placeholders(
                file_path_str,
                &version_dir,
                scope
            )?;
            let file_path = PathBuf::from(&expanded);

            if let Some(pkg_id) = pkg_id_opt
                && let Ok(conn) = db::open_connection("local")
                && let Ok(true) =
                    db::has_other_owners(&conn, file_path_str, pkg_id)
            {
                if !quiet {
                    println!(
                        "Keeping {} as it is still owned by other packages.",
                        file_path_str.dimmed()
                    );
                }
                continue;
            }

            // Use symlink_metadata so links are removed even when they dangle
            // or point to a non-empty directory; only the link is deleted.
            let Ok(meta) = fs::symlink_metadata(&file_path) else {
                continue;
            };

            if let Some(pkg_id) = pkg_id_opt
                && let Ok(conn) = db::open_connection("local")
                && let Ok(true) =
                    db::has_other_owners(&conn, file_path_str, pkg_id)
            {
                if !quiet {
                    println!(
                        "Keeping {} as it is still owned by other packages.",
                        file_path_str.dimmed()
                    );
                }
                continue;
            }

            if meta.file_type().is_symlink() {
                let _ = fs::remove_file(&file_path);
            } else if meta.is_dir() {
                // Only remove if empty to be safe
                if fs::read_dir(&file_path)
                    .is_ok_and(|mut e| e.next().is_none())
                {
                    let _ = fs::remove_dir(&file_path);
                }
            } else {
                let _ = fs::remove_file(&file_path);
            }
        }

        let manifest_path = version_dir.join(local::manifest_filename(
            sub_package_to_uninstall.as_deref()
        ));
        if manifest_path.exists() {
            fs::remove_file(&manifest_path)?;
        }
        // Remove the legacy YAML twin if an old install left one behind.
        let legacy_path = version_dir.join(format!(
            "manifest{}.yaml",
            sub_package_to_uninstall
                .as_deref()
                .map_or_else(String::new, |sub| format!("-{sub}"))
        ));
        if legacy_path != manifest_path && legacy_path.exists() {
            fs::remove_file(legacy_path)?;
        }

        if version_dir.exists() {
            let mut has_other_manifests = false;
            if let Ok(entries) = fs::read_dir(&version_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("manifest")
                        && std::path::Path::new(&name).extension().is_some_and(
                            |ext| {
                                ext.eq_ignore_ascii_case("json")
                                    || ext.eq_ignore_ascii_case("yaml")
                            }
                        )
                    {
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
                let _ = cleanup_service(&pkg.name, scope);
                if !quiet {
                    println!(
                        "Removing package store: {}",
                        package_dir.display()
                    );
                }
                fs::remove_dir_all(&package_dir)?;
            }
        }

        let parent_id = format!(
            "#{}@{}/{}@{}",
            manifest.registry_handle,
            manifest.repo,
            manifest.name,
            manifest.version
        );
        for dep_str in &manifest.installed_dependencies {
            if let Ok(dep) = dependencies::parse_dependency_string(dep_str)
                && dep.manager == "zoi"
            {
                let dep_req = resolve::parse_source_string(dep.package)?;
                let dep_matches =
                    local::find_installed_manifests_matching(&dep_req, scope)?;
                if dep_matches.len() == 1 {
                    let dep_manifest =
                        dep_matches.first().expect("Already checked length");
                    match local::get_package_dir(
                        dep_manifest.scope,
                        &dep_manifest.registry_handle,
                        &dep_manifest.repo,
                        &dep_manifest.name
                    ) {
                        Ok(dep_pkg_dir) => {
                            if let Err(e) = local::remove_dependent(
                                &dep_pkg_dir,
                                &parent_id
                            ) && !quiet
                            {
                                eprintln!(
                                    "Warning: failed to remove dependent link \
                                     for {}: {}",
                                    dep.package, e
                                );
                            }
                        }
                        Err(e) => {
                            if !quiet {
                                eprintln!(
                                    "Warning: failed to get package dir for \
                                     {}: {}",
                                    dep.package, e
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(hooks) = &pkg.hooks
            && let Err(e) =
                hooks::run_hooks(hooks, hooks::HookType::PostRemove, scope)
            && !quiet
        {
            eprintln!("{} post-remove hook failed: {}", "Warning:".yellow(), e);
        }
    }

    // The active version is gone from the store. Decide between complete
    // removal and rollback:
    // - Pinned uninstall (`package@1.2.0`) of the active version rolls back to
    //   the newest remaining stored version, store-only like `rollback`.
    // - Unpinned uninstall removes the package completely, including any other
    //   stored versions of the same (sub-)package.
    let remaining = if package_dir.exists() {
        list_stored_version_manifests(
            &package_dir,
            sub_package_to_uninstall.as_deref()
        )
    } else {
        Vec::new()
    };
    if !remaining.is_empty() {
        if request.version_spec.is_some() {
            let (prev_version, prev_manifest) =
                remaining.last().expect("remaining is not empty").clone();
            if !quiet {
                println!(
                    "Version {} still available in the store. Rolling back \
                     '{}' to {}...",
                    prev_version.cyan(),
                    pkg.name.bold(),
                    prev_version.green()
                );
            }
            match activate_stored_version(
                &package_dir,
                scope,
                &prev_version,
                &prev_manifest,
                quiet
            ) {
                Ok(()) => {
                    if let Ok(true) = telemetry::posthog_capture_event(
                        "uninstall",
                        &pkg,
                        env!("CARGO_PKG_VERSION"),
                        &manifest.registry_handle,
                        None
                    ) && !quiet
                    {
                        println!("{} telemetry sent", "Info:".green());
                    }
                    return Ok(manifest);
                }
                Err(e) => {
                    if needs_escalation {
                        eprintln!(
                            "{} rollback activation needs root and failed \
                             ({}); the old version stays in the store.",
                            "Warning:".yellow(),
                            e
                        );
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            // Complete removal: drop every remaining stored version of this
            // (sub-)package. Directories shared with other sub-packages are
            // kept by `remove_stored_version_dir`.
            for (stored_version, _) in &remaining {
                if let Err(e) = remove_stored_version_dir(
                    &package_dir,
                    stored_version,
                    sub_package_to_uninstall.as_deref(),
                    quiet
                ) && !quiet
                {
                    eprintln!(
                        "{} failed to remove stored version \
                         '{stored_version}': {}",
                        "Warning:".yellow(),
                        e
                    );
                }
            }
            if package_dir.exists() {
                let mut has_versions = false;
                if let Ok(entries) = fs::read_dir(&package_dir) {
                    for entry in entries.flatten() {
                        let name =
                            entry.file_name().to_string_lossy().to_string();
                        if name != "latest" && name != "dependents" {
                            has_versions = true;
                            break;
                        }
                    }
                }
                if !has_versions {
                    let _ = cleanup_service(&pkg.name, scope);
                    let latest_link = package_dir.join("latest");
                    if latest_link.is_symlink() {
                        let _ = fs::remove_file(&latest_link);
                    }
                    // Only remove the package dir when nothing but metadata
                    // (latest link, empty dependents) is left.
                    let mut leftovers = 0;
                    if let Ok(entries) = fs::read_dir(&package_dir) {
                        for entry in entries.flatten() {
                            let name =
                                entry.file_name().to_string_lossy().to_string();
                            if name == "dependents" {
                                let empty = entry
                                    .path()
                                    .read_dir()
                                    .is_ok_and(|mut e| e.next().is_none());
                                if empty {
                                    continue;
                                }
                            }
                            leftovers += 1;
                        }
                    }
                    if leftovers == 0 {
                        if !quiet {
                            println!(
                                "Removing package store: {}",
                                package_dir.display()
                            );
                        }
                        let _ = fs::remove_dir_all(&package_dir);
                    }
                }
            }
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
            Some(scope)
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
        None
    ) && !quiet
    {
        println!("{} telemetry sent", "Info:".green());
    }

    Ok(manifest)
}
