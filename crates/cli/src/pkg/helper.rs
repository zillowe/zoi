use crate::pkg::{
    install::{manifest, post_install, resolver::InstallNode},
    local, types,
};
use anyhow::{Result, anyhow};
use mlua::{Function, Lua, Table};
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn elevate_install_node(cmd: &crate::cmd::helper::ElevateInstallNodeCommand) -> Result<()> {
    let content = std::fs::read_to_string(&cmd.node_json)?;
    let node: InstallNode = serde_json::from_str(&content)?;

    let pkg = &node.pkg;
    let handle = &node.registry_handle;
    let sub_packages_vec = node.sub_package.clone().map(|s| vec![s]);

    let installed_files = crate::pkg::install::pkg_install::run(
        &cmd.archive,
        Some(pkg.scope),
        handle,
        Some(&node.version),
        cmd.yes,
        sub_packages_vec,
        cmd.link_bins,
        None,
    )?;

    if let types::InstallReason::Dependency { ref parent } = node.reason {
        let package_dir = local::get_package_dir(pkg.scope, handle, &pkg.repo, &pkg.name)?;
        local::add_dependent(&package_dir, parent)?;
    }

    let _ = post_install::install_manual_if_available(pkg, &node.version, handle, None);

    let manifest = manifest::create_manifest(
        pkg,
        node.reason.clone(),
        node.dependencies.clone(),
        Some(cmd.install_method.clone()),
        installed_files,
        handle,
        &node.chosen_options,
        &node.chosen_optionals,
        node.sub_package.clone(),
    )?;

    local::write_manifest(&manifest)?;
    local::persist_package_source(&manifest, Path::new(&node.source))?;

    Ok(())
}

pub fn elevate_uninstall(cmd: &crate::cmd::helper::ElevateUninstallCommand) -> Result<()> {
    let content = std::fs::read_to_string(&cmd.manifest_json)?;
    let manifest: types::InstallManifest = serde_json::from_str(&content)?;

    let handle = &manifest.registry_handle;
    let scope = manifest.scope;
    let package_dir = local::get_package_dir(scope, handle, &manifest.repo, &manifest.name)?;
    let version_dir = package_dir.join(&manifest.version);

    let pkg_lua_path = local::get_package_source_path(&manifest)?;
    let mut pkg_opt = None;
    if pkg_lua_path.exists() {
        let path_str = pkg_lua_path
            .to_str()
            .ok_or_else(|| anyhow!("Package path contains invalid UTF-8"))?;
        if let Ok(p) =
            crate::pkg::lua::parser::parse_lua_package(path_str, Some(&manifest.version), true)
        {
            pkg_opt = Some(p);
        }
    }

    if let Some(pkg) = &pkg_opt
        && let Some(hooks) = &pkg.hooks
    {
        let _ = crate::pkg::hooks::run_hooks(hooks, crate::pkg::hooks::HookType::PreRemove);
    }

    if pkg_lua_path.exists() {
        let lua = Lua::new();
        if crate::pkg::lua::functions::setup_lua_environment(
            &lua,
            &crate::pkg::utils::get_platform()?,
            Some(&manifest.version),
            pkg_lua_path.to_str(),
            None,
            manifest.sub_package.as_deref(),
            true,
        )
        .is_ok()
        {
            let lua_code = std::fs::read_to_string(&pkg_lua_path)?;
            if lua.load(&lua_code).exec().is_ok() {
                if let Ok(uninstall_fn) = lua.globals().get::<Function>("uninstall") {
                    let _ = uninstall_fn.call::<()>(());
                }

                if let Ok(uninstall_ops) = lua.globals().get::<Table>("__ZoiUninstallOperations") {
                    for op in uninstall_ops.sequence_values::<Table>() {
                        if let Ok(op) = op
                            && let Ok(op_type) = op.get::<String>("op")
                            && op_type == "zrm"
                        {
                            let mut path_to_remove: String = op.get("path").unwrap_or_default();
                            path_to_remove = path_to_remove
                                .replace("${pkgstore}", &version_dir.to_string_lossy());
                            if let Some(home_dir) = home::home_dir() {
                                path_to_remove = path_to_remove
                                    .replace("${usrhome}", &home_dir.to_string_lossy());
                            }
                            path_to_remove = path_to_remove.replace(
                                "${usrroot}",
                                &crate::pkg::sysroot::apply_sysroot(PathBuf::from("/"))
                                    .to_string_lossy(),
                            );

                            let path = std::path::PathBuf::from(path_to_remove);
                            if path.exists() {
                                if path.is_dir() {
                                    let _ = std::fs::remove_dir_all(path);
                                } else {
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(bins) = &manifest.bins {
        let bin_root = if cfg!(target_os = "windows") {
            Path::new("C:\\ProgramData\\zoi\\pkgs\\bin").to_path_buf()
        } else {
            Path::new("/usr/local/bin").to_path_buf()
        };

        for bin in bins {
            let symlink_path = bin_root.join(bin);
            if symlink_path.is_symlink() || symlink_path.exists() {
                let _ = std::fs::remove_file(&symlink_path);
            }
        }
    }

    for file_path_str in &manifest.installed_files {
        let file_path = Path::new(file_path_str);
        if file_path.exists() {
            if file_path.is_dir() {
                let _ = std::fs::remove_dir_all(file_path);
            } else {
                let _ = std::fs::remove_file(file_path);
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
        std::fs::remove_file(manifest_path)?;
    }

    if version_dir.exists() && std::fs::read_dir(&version_dir)?.next().is_none() {
        std::fs::remove_dir_all(version_dir)?;
    }

    if package_dir.exists() {
        let _ = crate::pkg::service::cleanup_service(&manifest.name, scope);
        if let Ok(mut entries) = std::fs::read_dir(&package_dir)
            && entries.next().is_none()
        {
            std::fs::remove_dir_all(package_dir)?;
        }
    }

    let parent_id = format!(
        "#{}@{}/{}@{}",
        manifest.registry_handle, manifest.repo, manifest.name, manifest.version
    );
    for dep_str in &manifest.installed_dependencies {
        if let Ok(dep) = crate::pkg::dependencies::parse_dependency_string(dep_str)
            && dep.manager == "zoi"
        {
            let dep_req = crate::pkg::resolve::parse_source_string(dep.package)?;
            let dep_matches =
                crate::pkg::local::find_installed_manifests_matching(&dep_req, scope)?;
            if dep_matches.len() == 1 {
                let dep_manifest = &dep_matches[0];
                if let Ok(dep_pkg_dir) = crate::pkg::local::get_package_dir(
                    dep_manifest.scope,
                    &dep_manifest.registry_handle,
                    &dep_manifest.repo,
                    &dep_manifest.name,
                ) {
                    let _ = crate::pkg::local::remove_dependent(&dep_pkg_dir, &parent_id);
                }
            }
        }
    }

    if let Some(pkg) = &pkg_opt
        && let Some(hooks) = &pkg.hooks
    {
        let _ = crate::pkg::hooks::run_hooks(hooks, crate::pkg::hooks::HookType::PostRemove);
    }

    Ok(())
}

pub enum HashType {
    Sha512,
    Sha256,
}

fn update_digest_from_reader<R: Read, D: Digest>(reader: &mut R, hasher: &mut D) -> Result<()> {
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(())
}

pub fn get_hash(source: &str, hash_type: HashType) -> Result<String> {
    let mut hasher_sha512 = Sha512::new();
    let mut hasher_sha256 = Sha256::new();

    if source.starts_with("http://") || source.starts_with("https://") {
        let client = crate::pkg::utils::get_http_client()?;
        let mut response = client.get(source).send()?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download file from URL: {}",
                response.status()
            ));
        }
        match hash_type {
            HashType::Sha512 => {
                update_digest_from_reader(&mut response, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                update_digest_from_reader(&mut response, &mut hasher_sha256)?;
            }
        }
    } else {
        let mut file = File::open(source)?;
        match hash_type {
            HashType::Sha512 => {
                update_digest_from_reader(&mut file, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                update_digest_from_reader(&mut file, &mut hasher_sha256)?;
            }
        }
    };

    let hash = match hash_type {
        HashType::Sha512 => hex::encode(hasher_sha512.finalize()),
        HashType::Sha256 => hex::encode(hasher_sha256.finalize()),
    };

    Ok(hash)
}

pub mod validate {
    use anyhow::{Result, anyhow};
    use colored::Colorize;
    use std::path::Path;

    pub fn run(file: &Path) -> Result<()> {
        if !file.exists() {
            return Err(anyhow!("File does not exist: {}", file.display()));
        }

        let content = std::fs::read_to_string(file)?;
        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        println!("{} Validating {}...", "::".bold().blue(), file.display());

        if file_name == "registries.json" {
            let _: crate::pkg::purl::CentralDbSpec = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid registries.json spec: {}", e))?;
            println!(
                "{} file is a valid registries.json spec.",
                "OK".bold().green()
            );
        } else if file_name == "repo.yaml" || file_name == "repo.yml" {
            let _: crate::pkg::types::RepoConfig = serde_yaml::from_str(&content)
                .map_err(|e| anyhow!("Invalid repo.yaml spec: {}", e))?;
            println!("{} file is a valid repo.yaml spec.", "OK".bold().green());
        } else if file_name == "advisories.json" {
            let _: crate::pkg::types::AdvisoryRegistry = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid advisories.json spec: {}", e))?;
            println!(
                "{} file is a valid advisories.json spec.",
                "OK".bold().green()
            );
        } else if file_name == "packages.json" {
            let _: crate::pkg::purl::RegistryIndex = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid packages.json spec: {}", e))?;
            println!(
                "{} file is a valid packages.json spec.",
                "OK".bold().green()
            );
        } else if file_name.ends_with(".sec.yaml") || file_name.ends_with(".sec.yml") {
            let _: crate::pkg::types::Advisory = serde_yaml::from_str(&content)
                .map_err(|e| anyhow!("Invalid security advisory (.sec.yaml) spec: {}", e))?;
            println!("{} file is a valid .sec.yaml spec.", "OK".bold().green());
        } else {
            if file.extension().and_then(|e| e.to_str()) == Some("json") {
                if serde_json::from_str::<crate::pkg::purl::CentralDbSpec>(&content).is_ok() {
                    println!("{} file matches registries.json spec.", "OK".bold().green());
                } else if serde_json::from_str::<crate::pkg::types::AdvisoryRegistry>(&content)
                    .is_ok()
                {
                    println!("{} file matches advisories.json spec.", "OK".bold().green());
                } else if serde_json::from_str::<crate::pkg::purl::RegistryIndex>(&content).is_ok()
                {
                    println!("{} file matches packages.json spec.", "OK".bold().green());
                } else {
                    return Err(anyhow!(
                        "File does not match any known Zoi JSON spec (registries.json, advisories.json, or packages.json)"
                    ));
                }
            } else if file.extension().and_then(|e| e.to_str()) == Some("yaml")
                || file.extension().and_then(|e| e.to_str()) == Some("yml")
            {
                if serde_yaml::from_str::<crate::pkg::types::RepoConfig>(&content).is_ok() {
                    println!("{} file matches repo.yaml spec.", "OK".bold().green());
                } else if serde_yaml::from_str::<crate::pkg::types::Advisory>(&content).is_ok() {
                    println!("{} file matches .sec.yaml spec.", "OK".bold().green());
                } else {
                    return Err(anyhow!(
                        "File does not match any known Zoi YAML spec (repo.yaml or .sec.yaml)"
                    ));
                }
            } else {
                return Err(anyhow!(
                    "Unsupported file extension. Please provide a .json or .yaml file"
                ));
            }
        }

        Ok(())
    }
}
