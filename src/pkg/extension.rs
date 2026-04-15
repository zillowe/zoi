use crate::pkg::{config, local, resolve, types};
use anyhow::{Result, anyhow};
use mlua::LuaSerdeExt;
use std::fs;
use std::path::PathBuf;

const EXTENSION_STATE_FILE: &str = "extension-state.yaml";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ExtensionState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_default_registry: Option<types::Registry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_file_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    installed_extension: Option<types::ExtensionInfo>,
}

fn get_extension_state_path(manifest: &types::InstallManifest) -> Result<PathBuf> {
    let version_dir = local::get_package_version_dir(
        manifest.scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
        &manifest.version,
    )?;
    Ok(version_dir.join(EXTENSION_STATE_FILE))
}

fn write_extension_state(
    manifest: &types::InstallManifest,
    extension_state: &ExtensionState,
) -> Result<()> {
    let state_path = get_extension_state_path(manifest)?;
    fs::write(state_path, serde_yaml::to_string(extension_state)?)?;
    Ok(())
}

fn read_extension_state(manifest: &types::InstallManifest) -> Result<Option<ExtensionState>> {
    let state_path = get_extension_state_path(manifest)?;
    if !state_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(state_path)?;
    Ok(Some(serde_yaml::from_str(&content)?))
}

fn restore_default_registry(
    saved_state: Option<&ExtensionState>,
    added_registry_url: &str,
) -> Result<()> {
    if let Some(saved_state) = saved_state {
        return config::set_user_default_registry(saved_state.previous_default_registry.clone());
    }

    let user_config = config::read_user_config()?;
    let should_clear = user_config
        .default_registry
        .as_ref()
        .map(|registry| registry.url == added_registry_url)
        .unwrap_or(false);
    if should_clear {
        config::set_user_default_registry(None)?;
    }
    Ok(())
}

fn extension_state_requires_persistence(extension_state: &ExtensionState) -> bool {
    extension_state.previous_default_registry.is_some()
        || extension_state.project_file_path.is_some()
        || extension_state.installed_extension.is_some()
}

fn get_project_file_path(saved_state: Option<&ExtensionState>) -> PathBuf {
    saved_state
        .and_then(|state| state.project_file_path.clone())
        .unwrap_or_else(|| PathBuf::from("zoi.yaml"))
}

fn get_repo_name_from_url(url: &str) -> &str {
    url.trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or_default()
        .trim_end_matches(".git")
}

fn revert_extension_change(
    change: &types::ExtensionChange,
    saved_state: Option<&ExtensionState>,
) -> Result<()> {
    match change {
        types::ExtensionChange::RepoGit { add } => {
            let repo_name = get_repo_name_from_url(add);
            if !repo_name.is_empty() {
                config::remove_git_repo(repo_name)?;
            }
        }
        types::ExtensionChange::RegistryRepo { add } => {
            restore_default_registry(saved_state, add)?;
        }
        types::ExtensionChange::RegistryAdd { add } => {
            config::remove_added_registry(add)?;
        }
        types::ExtensionChange::RepoAdd { add } => {
            config::remove_repo(add)?;
        }
        types::ExtensionChange::Project { add: _ } => {
            let project_file_path = get_project_file_path(saved_state);
            if project_file_path.exists() {
                fs::remove_file(project_file_path)?;
            }
        }
        types::ExtensionChange::Pgp { name, key: _ } => {
            crate::pkg::pgp::remove_key_by_name(name)?;
        }
        types::ExtensionChange::Plugin { name, script: _ } => {
            let plugin_dir = crate::pkg::plugin::get_plugin_dir()?;
            let plugin_path = plugin_dir.join(format!("{}.lua", name));
            if plugin_path.exists() {
                fs::remove_file(plugin_path)?;
            }
        }
        types::ExtensionChange::Hook { name, content: _ } => {
            let hooks_dir = crate::pkg::hooks::global::get_user_hooks_dir()?;
            let hook_path = hooks_dir.join(format!("{}.hook.yaml", name));
            if hook_path.exists() {
                fs::remove_file(hook_path)?;
            }
        }
    }
    Ok(())
}

pub fn add(
    ext_name: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("Adding extension: {}", ext_name);

    let (pkg, _, _, _, registry_handle, _) =
        resolve::resolve_package_and_version(ext_name, false, yes)?;

    if pkg.package_type != types::PackageType::Extension {
        return Err(anyhow!("'{}' is not an extension package.", ext_name));
    }

    let pkg_val = plugin_manager
        .lua
        .to_value(&pkg)
        .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
    plugin_manager.trigger_hook("on_pre_extension_add", Some(pkg_val))?;

    let extension_info = if let Some(extension_info) = pkg.extension {
        extension_info
    } else {
        return Err(anyhow!(
            "'{}' is an extension package but contains no extension data.",
            ext_name
        ));
    };
    if extension_info.extension_type != "zoi" {
        return Err(anyhow!(
            "Unsupported extension type: {}",
            extension_info.extension_type
        ));
    }
    let has_registry_repo_change = extension_info
        .changes
        .iter()
        .any(|change| matches!(change, types::ExtensionChange::RegistryRepo { .. }));
    let has_project_change = extension_info
        .changes
        .iter()
        .any(|change| matches!(change, types::ExtensionChange::Project { .. }));
    let previous_default_registry = if has_registry_repo_change {
        config::read_user_config()?.default_registry.clone()
    } else {
        None
    };
    let project_file_path = if has_project_change {
        Some(std::env::current_dir()?.join("zoi.yaml"))
    } else {
        None
    };
    let extension_state = ExtensionState {
        previous_default_registry,
        project_file_path,
        installed_extension: Some(extension_info.clone()),
    };

    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().unwrap_or_default(),
        sub_package: None,
        repo: pkg.repo.clone(),
        registry_handle: registry_handle.unwrap_or_default(),
        package_type: pkg.package_type,
        reason: types::InstallReason::Direct,
        scope: pkg.scope,
        bins: None,
        conflicts: None,
        replaces: None,
        provides: None,
        backup: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: None,
        service: None,
        installed_files: vec![],
        installed_size: pkg.installed_size,
    };
    let mut wrote_manifest = false;
    let mut applied_changes = Vec::new();
    let add_result = (|| -> Result<()> {
        if extension_state_requires_persistence(&extension_state) {
            local::write_manifest(&manifest)?;
            wrote_manifest = true;
            write_extension_state(&manifest, &extension_state)?;
        }

        println!("Applying extension changes...");
        for change in &extension_info.changes {
            match change {
                types::ExtensionChange::RepoGit { add } => {
                    println!("Adding git repository: {}", add);
                    config::clone_git_repo(add)?;
                }
                types::ExtensionChange::RegistryRepo { add } => {
                    println!("Setting registry to: {}", add);
                    config::set_default_registry(add)?;
                }
                types::ExtensionChange::RegistryAdd { add } => {
                    println!("Adding registry: {}", add);
                    config::add_added_registry(add)?;
                }
                types::ExtensionChange::RepoAdd { add } => {
                    println!("Adding repository: {}", add);
                    config::add_repo(add)?;
                }
                types::ExtensionChange::Project { add } => {
                    let project_file_path = get_project_file_path(Some(&extension_state));
                    println!("Creating {}...", project_file_path.display());
                    if project_file_path.exists() {
                        return Err(anyhow!(
                            "A 'zoi.yaml' file already exists at '{}'. Please remove it first.",
                            project_file_path.display()
                        ));
                    }
                    fs::write(&project_file_path, add)?;
                }
                types::ExtensionChange::Pgp { name, key } => {
                    println!("Adding PGP key: {} from {}", name, key);
                    if key.starts_with("http") {
                        crate::pkg::pgp::add_key_from_url(key, name)?;
                    } else {
                        crate::pkg::pgp::add_key_from_fingerprint(key, name)?;
                    }
                }
                types::ExtensionChange::Plugin { name, script } => {
                    println!("Adding plugin: {}", name);
                    let plugin_dir = crate::pkg::plugin::get_plugin_dir()?;
                    let plugin_path = plugin_dir.join(format!("{}.lua", name));
                    fs::write(plugin_path, script)?;
                }
                types::ExtensionChange::Hook { name, content } => {
                    println!("Adding global hook: {}", name);
                    let hooks_dir = crate::pkg::hooks::global::get_user_hooks_dir()?;
                    let hook_path = hooks_dir.join(format!("{}.hook.yaml", name));
                    fs::write(hook_path, content)?;
                }
            }
            applied_changes.push(change.clone());
        }
        if !wrote_manifest {
            local::write_manifest(&manifest)?;
            wrote_manifest = true;
        }
        Ok(())
    })();
    if let Err(error) = add_result {
        for change in applied_changes.iter().rev() {
            if let Err(rollback_error) = revert_extension_change(change, Some(&extension_state)) {
                eprintln!(
                    "Warning: failed to roll back extension change {:?}: {}",
                    change, rollback_error
                );
            }
        }
        if wrote_manifest
            && let Ok(package_dir) = local::get_package_dir(
                manifest.scope,
                &manifest.registry_handle,
                &manifest.repo,
                &manifest.name,
            )
        {
            let _ = fs::remove_dir_all(package_dir);
        }
        return Err(error);
    }

    let manifest_val = plugin_manager
        .lua
        .to_value(&manifest)
        .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
    plugin_manager.trigger_hook_nonfatal("on_post_extension_add", Some(manifest_val));

    println!("Successfully added extension '{}'.", ext_name);

    Ok(())
}

pub fn remove(
    ext_name: &str,
    yes: bool,
    plugin_manager: &crate::pkg::plugin::PluginManager,
) -> Result<()> {
    println!("Removing extension: {}", ext_name);

    let (pkg, _, _, _, _, _) = resolve::resolve_package_and_version(ext_name, false, yes)?;

    let (manifest, scope) = if let Some(m) =
        local::is_package_installed(&pkg.name, None, types::Scope::User)?
    {
        (m, types::Scope::User)
    } else if let Some(m) = local::is_package_installed(&pkg.name, None, types::Scope::System)? {
        (m, types::Scope::System)
    } else {
        return Err(anyhow!("Extension '{}' is not installed.", ext_name));
    };

    let manifest_val = plugin_manager
        .lua
        .to_value(&manifest)
        .map_err(|e: mlua::Error| anyhow!(e.to_string()))?;
    plugin_manager.trigger_hook("on_pre_extension_remove", Some(manifest_val.clone()))?;

    if manifest.package_type != types::PackageType::Extension {
        return Err(anyhow!("'{}' is not an extension package.", ext_name));
    }

    let extension_state = read_extension_state(&manifest)?;
    let extension_info = extension_state
        .as_ref()
        .and_then(|state| state.installed_extension.clone())
        .or(pkg.extension);

    if let Some(extension_info) = extension_info {
        if extension_info.extension_type != "zoi" {
            return Err(anyhow!(
                "Unsupported extension type: {}",
                extension_info.extension_type
            ));
        }

        println!("Reverting extension changes...");
        for change in extension_info.changes.iter().rev() {
            match change {
                types::ExtensionChange::RepoGit { add } => {
                    let repo_name = get_repo_name_from_url(add);
                    if !repo_name.is_empty() {
                        println!("Removing git repository: {}", repo_name);
                        if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                            eprintln!("Warning: failed to remove git repo '{}': {}", repo_name, e);
                        }
                    }
                }
                types::ExtensionChange::RegistryRepo { add: _ } => {
                    println!("Restoring previous default registry");
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to restore default registry: {}", e);
                    }
                }
                types::ExtensionChange::RegistryAdd { add } => {
                    println!("Removing registry: {}", add);
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to remove registry '{}': {}", add, e);
                    }
                }
                types::ExtensionChange::RepoAdd { add } => {
                    println!("Removing repository: {}", add);
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to remove repo '{}': {}", add, e);
                    }
                }
                types::ExtensionChange::Project { add: _ } => {
                    let project_file_path = get_project_file_path(extension_state.as_ref());
                    println!("Removing {}...", project_file_path.display());
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!(
                            "Warning: failed to remove '{}': {}",
                            project_file_path.display(),
                            e
                        );
                    }
                }
                types::ExtensionChange::Pgp { name, key: _ } => {
                    println!("Removing PGP key: {}", name);
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to remove PGP key '{}': {}", name, e);
                    }
                }
                types::ExtensionChange::Plugin { name, script: _ } => {
                    println!("Removing plugin: {}", name);
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to remove plugin '{}': {}", name, e);
                    }
                }
                types::ExtensionChange::Hook { name, content: _ } => {
                    println!("Removing global hook: {}", name);
                    if let Err(e) = revert_extension_change(change, extension_state.as_ref()) {
                        eprintln!("Warning: failed to remove global hook '{}': {}", name, e);
                    }
                }
            }
        }
    } else {
        return Err(anyhow!(
            "'{}' is an extension package but contains no extension data.",
            ext_name
        ));
    }

    let package_dir = local::get_package_dir(
        scope,
        &manifest.registry_handle,
        &manifest.repo,
        &manifest.name,
    )?;

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)?;
    }

    plugin_manager.trigger_hook_nonfatal("on_post_extension_remove", Some(manifest_val));

    println!("Successfully removed extension '{}'.", ext_name);

    Ok(())
}
