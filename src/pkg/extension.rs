use crate::pkg::{config, local, resolve, types};
use std::error::Error;
use std::fs;

pub fn add(ext_name: &str, _yes: bool) -> Result<(), Box<dyn Error>> {
    println!("Adding extension: {}", ext_name);

    let (pkg, _, _, _, registry_handle) = resolve::resolve_package_and_version(ext_name)?;

    if pkg.package_type != types::PackageType::Extension {
        return Err(format!("'{}' is not an extension package.", ext_name).into());
    }

    if let Some(extension_info) = pkg.extension {
        if extension_info.extension_type != "zoi" {
            return Err(format!(
                "Unsupported extension type: {}",
                extension_info.extension_type
            )
            .into());
        }

        println!("Applying extension changes...");
        for change in extension_info.changes {
            match change {
                types::ExtensionChange::RepoGit { add } => {
                    println!("Adding git repository: {}", add);
                    config::clone_git_repo(&add)?;
                }
                types::ExtensionChange::RegistryRepo { add } => {
                    println!("Setting registry to: {}", add);
                    config::set_default_registry(&add)?;
                }
                types::ExtensionChange::RepoAdd { add } => {
                    println!("Adding repository: {}", add);
                    config::add_repo(&add)?;
                }
                types::ExtensionChange::Project { add } => {
                    println!("Creating zoi.yaml...");
                    if std::path::Path::new("zoi.yaml").exists() {
                        return Err("A 'zoi.yaml' file already exists in the current directory. Please remove it first."
                            .into());
                    }
                    fs::write("zoi.yaml", add)?;
                }
            }
        }
    } else {
        return Err(format!(
            "'{}' is an extension package but contains no extension data.",
            ext_name
        )
        .into());
    }

    let manifest = types::InstallManifest {
        name: pkg.name.clone(),
        version: pkg.version.clone().unwrap_or_default(),
        repo: pkg.repo.clone(),
        registry_handle: registry_handle.unwrap_or_default(),
        package_type: pkg.package_type,
        installed_at: chrono::Utc::now().to_rfc3339(),
        reason: types::InstallReason::Direct,
        scope: pkg.scope,
        bins: None,
        conflicts: None,
        installed_dependencies: vec![],
        chosen_options: vec![],
        chosen_optionals: vec![],
        install_method: None,
        installed_files: vec![],
    };
    local::write_manifest(&manifest)?;

    println!("Successfully added extension '{}'.", ext_name);

    Ok(())
}

pub fn remove(ext_name: &str, _yes: bool) -> Result<(), Box<dyn Error>> {
    println!("Removing extension: {}", ext_name);

    let (manifest, scope) =
        if let Some(m) = local::is_package_installed(ext_name, types::Scope::User)? {
            (m, types::Scope::User)
        } else if let Some(m) = local::is_package_installed(ext_name, types::Scope::System)? {
            (m, types::Scope::System)
        } else {
            return Err(format!("Extension '{}' is not installed.", ext_name).into());
        };

    let (pkg, _, _, _, _) = resolve::resolve_package_and_version(ext_name)?;

    if pkg.package_type != types::PackageType::Extension {
        return Err(format!("'{}' is not an extension package.", ext_name).into());
    }

    if let Some(extension_info) = pkg.extension {
        if extension_info.extension_type != "zoi" {
            return Err(format!(
                "Unsupported extension type: {}",
                extension_info.extension_type
            )
            .into());
        }

        println!("Reverting extension changes...");
        for change in extension_info.changes.iter().rev() {
            match change {
                types::ExtensionChange::RepoGit { add } => {
                    let repo_name = add
                        .trim_end_matches('/')
                        .split('/')
                        .next_back()
                        .unwrap_or("")
                        .trim_end_matches(".git");
                    if !repo_name.is_empty() {
                        println!("Removing git repository: {}", repo_name);
                        if let Err(e) = config::remove_git_repo(repo_name) {
                            eprintln!("Warning: failed to remove git repo '{}': {}", repo_name, e);
                        }
                    }
                }
                types::ExtensionChange::RegistryRepo { add: _ } => {
                    let default_registry = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoidberg.git";
                    println!("Setting registry back to default");
                    if let Err(e) = config::set_default_registry(default_registry) {
                        eprintln!("Warning: failed to set registry to default: {}", e);
                    }
                }
                types::ExtensionChange::RepoAdd { add } => {
                    println!("Removing repository: {}", add);
                    if let Err(e) = config::remove_repo(add) {
                        eprintln!("Warning: failed to remove repo '{}': {}", add, e);
                    }
                }
                types::ExtensionChange::Project { add: _ } => {
                    println!("Removing zoi.yaml...");
                    if let Err(e) = fs::remove_file("zoi.yaml") {
                        eprintln!("Warning: failed to remove 'zoi.yaml': {}", e);
                    }
                }
            }
        }
    } else {
        return Err(format!(
            "'{}' is an extension package but contains no extension data.",
            ext_name
        )
        .into());
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

    println!("Successfully removed extension '{}'.", ext_name);

    Ok(())
}
