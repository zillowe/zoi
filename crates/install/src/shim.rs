use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Result, anyhow};
use walkdir::WalkDir;
use zoi_core::config;
use zoi_core::types::Scope;
use zoi_core::utils::{ask_for_confirmation, symlink_file};
use zoi_db as db;
use zoi_plugins::PluginManager;
use zoi_project as project;
use zoi_resolver::{local, resolve};
#[cfg(target_os = "linux")]
use zoi_sandbox as sandbox;

/// Runs a binary through the Zoi shim mechanism.
///
/// # Errors
///
/// Returns an error if the binary cannot be resolved, if sandboxing fails,
/// or if execution of the binary fails.
pub fn run_shim(
    bin_name: &str,
    args: Vec<String,>,
    plugin_manager: Option<&PluginManager,>,
    auto_install: Option<&dyn Fn(&str, &str,) -> Result<(),>,>,
) -> Result<(),> {
    let bin_path =
        resolve_to_installed_bin(bin_name, plugin_manager, auto_install,)?;

    #[cfg(target_os = "linux")]
    {
        let mut current = bin_path.parent();
        let mut manifest: Option<zoi_core::types::InstallManifest,> = None;
        let mut pkg_version_dir = None;

        while let Some(path,) = current {
            let mut manifest_path = None;
            if let Ok(entries,) = fs::read_dir(path,) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("manifest",)
                        && std::path::Path::new(&name,).extension().is_some_and(
                            |ext| ext.eq_ignore_ascii_case("yaml",),
                        )
                    {
                        manifest_path = Some(entry.path(),);
                        break;
                    }
                }
            }

            if let Some(mp,) = manifest_path
                && let Ok(content,) = fs::read_to_string(mp,)
                && let Ok(m,) = serde_yaml::from_str::<
                    zoi_core::types::InstallManifest,
                >(&content,)
            {
                manifest = Some(m,);
                pkg_version_dir = Some(path.to_path_buf(),);
                break;
            }
            current = path.parent();
        }

        if let Some(m,) = manifest
            && let Some(sandbox,) = m.sandbox
            && sandbox.enabled
            && let Some(version_dir,) = pkg_version_dir
        {
            use std::os::unix::process::CommandExt;
            let mut cmd = sandbox::wrap_command(
                &bin_path,
                &args,
                &sandbox,
                &version_dir,
            )?;
            let err = cmd.exec();
            return Err(anyhow!(
                "Failed to execute sandboxed binary '{bin_name}': {err}"
            ),);
        }
    }

    let mut cmd = std::process::Command::new(bin_path,);
    cmd.args(args,);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(anyhow!("Failed to execute binary '{bin_name}': {err}"),)
    }

    #[cfg(windows)]
    {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        std::process::exit(status.code().unwrap_or(0,),);
    }
}

/// Resolves a binary name to its installed path in the Zoi store.
///
/// # Errors
///
/// Returns an error if the binary cannot be found in the store, if the
/// desired version cannot be determined, or if automatic installation fails.
pub fn resolve_to_installed_bin(
    bin_name: &str,
    plugin_manager: Option<&PluginManager,>,
    auto_install: Option<&dyn Fn(&str, &str,) -> Result<(),>,>,
) -> Result<PathBuf,> {
    let desired_version = get_desired_version(bin_name, plugin_manager,)?;

    let providers = db::find_provides("local", bin_name,)?;

    if let Some(version,) = &desired_version {
        for (pkg, _,) in &providers {
            if let Some(path,) =
                search_store_for_version(&pkg.name, version, bin_name,)?
            {
                return Ok(path,);
            }
        }

        if let Some(install,) = auto_install
            && ask_for_confirmation(
                &format!(
                    "Binary '{bin_name}' v{version} is required but not \
                     installed. Install it now?"
                ),
                false,
            )
        {
            install(bin_name, version,)?;
            let providers = db::find_provides("local", bin_name,)?;
            for (pkg, _,) in &providers {
                if let Some(path,) =
                    search_store_for_version(&pkg.name, version, bin_name,)?
                {
                    return Ok(path,);
                }
            }
        }
    }

    if providers.is_empty() {
        return Err(anyhow!(
            "No installed package provides binary '{bin_name}'. Run 'zoi \
             provides {bin_name}' to find providers."
        ),);
    }

    if let Some(version,) = &desired_version {
        for (pkg, _,) in &providers {
            if let Some(path,) =
                search_store_for_version(&pkg.name, version, bin_name,)?
            {
                return Ok(path,);
            }
        }
    }

    let Some((pkg, _,),) = providers.first() else {
        return Err(anyhow!(
            "No installed package provides binary '{bin_name}'."
        ),);
    };

    if let Some(path,) =
        search_store_for_version(&pkg.name, "latest", bin_name,)?
    {
        return Ok(path,);
    }

    let version = pkg.version.as_deref().ok_or_else(|| {
        anyhow!("Package '{}' has no version info in DB", pkg.name)
    },)?;

    if let Some(path,) =
        search_store_for_version(&pkg.name, version, bin_name,)?
    {
        return Ok(path,);
    }

    for scope in [Scope::Project, Scope::User, Scope::System,] {
        let store_root = local::get_store_base_dir(scope,)?;
        if !store_root.exists() {
            continue;
        }

        for entry in fs::read_dir(store_root,)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name,) = path.file_name().and_then(|s| s.to_str(),)
                && dir_name.ends_with(&format!("-{}", pkg.name),)
            {
                let latest_dir = path.join("latest",);
                if latest_dir.exists()
                    && let Some(p,) = find_bin_in_dir(&latest_dir, bin_name,)
                {
                    return Ok(p,);
                }
            }
        }
    }

    Err(anyhow!(
        "Could not locate binary '{bin_name}' in the Zoi store. Try \
         reinstalling the provider package."
    ),)
}

/// Finds the version for a binary in a `.tool-versions` file.
fn find_tool_versions_version(bin_name: &str,) -> Result<Option<String,>,> {
    let mut current_dir = env::current_dir()?;
    loop {
        let tool_versions_path = current_dir.join(".tool-versions",);
        if tool_versions_path.exists() {
            let content = fs::read_to_string(&tool_versions_path,)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#',) {
                    continue;
                }
                let mut parts = line.split_whitespace();
                if let (Some(p0,), Some(p1,),) = (parts.next(), parts.next(),)
                    && p0 == bin_name
                {
                    return Ok(Some(p1.to_string(),),);
                }
            }
        }
        if !current_dir.pop() {
            break;
        }
    }
    Ok(None,)
}

/// Gets the desired version for a binary from various sources.
fn get_desired_version(
    bin_name: &str,
    plugin_manager: Option<&PluginManager,>,
) -> Result<Option<String,>,> {
    let env_var_name =
        format!("ZOI_{}_VERSION", bin_name.to_uppercase().replace('-', "_"));
    if let Ok(v,) = env::var(&env_var_name,) {
        return Ok(Some(v,),);
    }

    if let Some(pm,) = plugin_manager
        && let Ok(Some(v,),) = pm.trigger_resolve_shim_version(bin_name,)
    {
        return Ok(Some(v,),);
    }

    if let Ok(project_cfg,) = project::config::load() {
        for pkg_spec in project_cfg.pkgs {
            if let Ok(req,) = resolve::parse_source_string(&pkg_spec,) {
                let is_match = req.name == bin_name || {
                    if let Ok(providers,) =
                        db::find_provides("local", bin_name,)
                    {
                        providers.iter().any(|(p, _,)| p.name == req.name,)
                    } else {
                        false
                    }
                };

                if is_match && let Some(v,) = req.version_spec {
                    return Ok(Some(v,),);
                }
            }
        }
    }

    if let Ok(Some(v,),) = find_tool_versions_version(bin_name,) {
        return Ok(Some(v,),);
    }

    let cfg = config::read_config()?;
    if let Some(v,) = cfg.versions.get(bin_name,) {
        return Ok(Some(v.clone(),),);
    }

    Ok(None,)
}

/// Searches the Zoi store for a specific version of a package that provides a
/// binary.
fn search_store_for_version(
    pkg_name: &str,
    version: &str,
    bin_name: &str,
) -> Result<Option<PathBuf,>,> {
    for scope in [Scope::Project, Scope::User, Scope::System,] {
        let store_root = local::get_store_base_dir(scope,)?;
        if !store_root.exists() {
            continue;
        }

        for entry in fs::read_dir(store_root,)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(dir_name,) = path.file_name().and_then(|s| s.to_str(),)
                && dir_name.ends_with(&format!("-{pkg_name}"),)
            {
                let latest_dir = path.join("latest",);
                if latest_dir.exists()
                    && (version == "latest" || version.is_empty())
                    && let Some(p,) = find_bin_in_dir(&latest_dir, bin_name,)
                {
                    return Ok(Some(p,),);
                }

                if version != "latest" && !version.is_empty() {
                    let version_dir = path.join(version,);
                    if version_dir.exists()
                        && let Some(p,) =
                            find_bin_in_dir(&version_dir, bin_name,)
                    {
                        return Ok(Some(p,),);
                    }

                    for v_entry in fs::read_dir(&path,)? {
                        let v_entry = v_entry?;
                        let v_name =
                            v_entry.file_name().to_string_lossy().to_string();
                        if v_name.starts_with(version,)
                            && v_name != "latest"
                            && v_name != "dependents"
                        {
                            let v_dir = path.join(v_name,);
                            if let Some(p,) = find_bin_in_dir(&v_dir, bin_name,)
                            {
                                return Ok(Some(p,),);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(None,)
}

/// Finds a binary in a directory, checking `bin/` first and then walking the
/// directory.
fn find_bin_in_dir(dir: &std::path::Path, bin_name: &str,) -> Option<PathBuf,> {
    let bin_path = dir.join("bin",).join(bin_name,);
    if bin_path.exists() {
        return Some(bin_path,);
    }

    for entry in WalkDir::new(dir,)
        .into_iter()
        .filter_map(std::result::Result::ok,)
    {
        if entry.file_type().is_file()
            && entry.file_name().to_string_lossy() == bin_name
        {
            return Some(entry.path().to_path_buf(),);
        }
    }
    None
}

/// Creates a shim for the current Zoi executable at the specified path.
///
/// # Errors
///
/// Returns an error if the current executable path cannot be determined
/// or if creating the symbolic link fails.
pub fn create_shim(link_path: &std::path::Path,) -> Result<(),> {
    let zoi_exe = env::current_exe()?;
    symlink_file(&zoi_exe, link_path,)
        .map_err(|e| anyhow!("Failed to create shim: {e}"),)
}
