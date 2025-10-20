use crate::pkg::{cache, local, resolve, types};
use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use tar::Archive;
use zstd::stream::read::Decoder as ZstdDecoder;

fn ensure_binary_is_cached(pkg: &types::Package, upstream: bool) -> Result<PathBuf> {
    let cache_dir = cache::get_cache_root()?;
    let binary_filename = if cfg!(target_os = "windows") {
        format!("{}.exe", pkg.name)
    } else {
        pkg.name.clone()
    };
    let bin_path = cache_dir.join(&binary_filename);

    if upstream && bin_path.exists() {
        fs::remove_file(&bin_path)?;
    }

    if bin_path.exists() {
        println!("Using cached binary for '{}'.", pkg.name.cyan());
        return Ok(bin_path);
    }

    if !pkg.types.contains(&"pre-compiled".to_string()) {
        return Err(anyhow!(
            "zoi exec only works with 'pre-compiled' package types."
        ));
    }

    println!(
        "No cached binary found for '{}'. Downloading pre-built package...",
        pkg.name.cyan()
    );
    fs::create_dir_all(&cache_dir)?;

    let db_path = resolve::get_db_root()?;
    let config = crate::pkg::config::read_config()?;
    let repo_config = if let Some(handle) = config.default_registry.as_ref().map(|r| &r.handle) {
        crate::pkg::config::read_repo_config(&db_path.join(handle)).ok()
    } else {
        None
    };

    if let Some(repo_config) = repo_config {
        let mut pkg_links_to_try = Vec::new();
        if let Some(main_pkg) = repo_config.pkg.iter().find(|p| p.link_type == "main") {
            pkg_links_to_try.push(main_pkg.clone());
        }
        pkg_links_to_try.extend(
            repo_config
                .pkg
                .iter()
                .filter(|p| p.link_type == "mirror")
                .cloned(),
        );

        for pkg_link in pkg_links_to_try {
            let platform = utils::get_platform()?;
            let (os, arch) = (
                platform.split('-').next().unwrap_or(""),
                platform.split('-').nth(1).unwrap_or(""),
            );
            let url_dir = pkg_link
                .url
                .replace("{os}", os)
                .replace("{arch}", arch)
                .replace("{version}", pkg.version.as_deref().unwrap_or(""))
                .replace("{repo}", &pkg.repo);

            let archive_filename = format!("{}.pkg.tar.zst", pkg.name);
            let final_url = format!("{}/{}", url_dir.trim_end_matches('/'), archive_filename);

            println!(
                "Attempting to download pre-built package from: {}",
                final_url.cyan()
            );

            let temp_dir = tempfile::Builder::new().prefix("zoi-exec-dl-").tempdir()?;
            let temp_archive_path = temp_dir.path().join(&archive_filename);

            if crate::pkg::install::util::download_file_with_progress(
                &final_url,
                &temp_archive_path,
                None,
            )
            .is_ok()
            {
                let downloaded_data = fs::read(&temp_archive_path)?;
                let temp_ext_dir = tempfile::Builder::new().prefix("zoi-exec-ext").tempdir()?;
                let mut archive = Archive::new(ZstdDecoder::new(Cursor::new(downloaded_data))?);
                archive.unpack(temp_ext_dir.path())?;

                let bin_dir_in_archive = temp_ext_dir.path().join("data/pkgstore/bin");
                if bin_dir_in_archive.exists()
                    && let Some(bin_name) = &pkg.bins.as_ref().and_then(|b| b.first())
                {
                    let bin_in_archive = bin_dir_in_archive.join(bin_name);
                    if bin_in_archive.exists() {
                        let final_bin_path = cache_dir.join(bin_name);
                        fs::copy(&bin_in_archive, &final_bin_path)?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            fs::set_permissions(
                                &final_bin_path,
                                fs::Permissions::from_mode(0o755),
                            )?;
                        }
                        println!("Binary cached successfully.");
                        return Ok(final_bin_path);
                    }
                }
            }
        }
    }

    Err(anyhow!("Could not download pre-built package for exec."))
}

fn find_executable(
    pkg: &types::Package,
    upstream: bool,
    cache_only: bool,
    local_only: bool,
    registry_handle: Option<&str>,
) -> Result<PathBuf> {
    let handle = registry_handle.unwrap_or("local");

    if upstream {
        return ensure_binary_is_cached(pkg, true);
    }

    let scopes_to_check = if local_only {
        vec![types::Scope::Project]
    } else {
        vec![
            types::Scope::Project,
            types::Scope::User,
            types::Scope::System,
        ]
    };

    for scope in scopes_to_check {
        if let Ok(package_dir) = local::get_package_dir(scope, handle, &pkg.repo, &pkg.name) {
            let latest_path = package_dir.join("latest");
            if latest_path.exists() {
                let binary_filename = if cfg!(target_os = "windows") {
                    format!("{}.exe", pkg.name)
                } else {
                    pkg.name.clone()
                };
                let bin_path = latest_path.join("bin").join(binary_filename);
                if bin_path.exists() {
                    let scope_str = match scope {
                        types::Scope::Project => "project-local",
                        types::Scope::User => "user",
                        types::Scope::System => "system",
                    };
                    println!("Using {} binary for '{}'.", scope_str, pkg.name.cyan());
                    return Ok(bin_path);
                }
            }
        }
    }

    if local_only {
        return Err(anyhow!("No local project binary found."));
    }

    if cache_only {
        let cache_dir = cache::get_cache_root()?;
        let binary_filename = if cfg!(target_os = "windows") {
            format!("{}.exe", pkg.name)
        } else {
            pkg.name.clone()
        };
        let bin_path = cache_dir.join(&binary_filename);
        if bin_path.exists() {
            println!("Using cached binary for '{}'.", pkg.name.cyan());
            return Ok(bin_path);
        }
        return Err(anyhow!("No cached binary found."));
    }

    ensure_binary_is_cached(pkg, false)
}

pub fn run(
    source: &str,
    args: Vec<String>,
    upstream: bool,
    cache_only: bool,
    local_only: bool,
) -> Result<()> {
    let resolved_source = resolve::resolve_source(source)?;

    if let Some(repo_name) = &resolved_source.repo_name {
        utils::print_repo_warning(repo_name);
    }

    let mut pkg: types::Package =
        crate::pkg::lua::parser::parse_lua_package(resolved_source.path.to_str().unwrap(), None)?;

    if let Some(repo_name) = resolved_source.repo_name.clone() {
        pkg.repo = repo_name;
    }

    if pkg.package_type == types::PackageType::App {
        return Err(anyhow!(
            "This package is an 'app' template. Use 'zoi create <pkg> <appName>' to create an app from it."
        ));
    }

    let bin_path = find_executable(
        &pkg,
        upstream,
        cache_only,
        local_only,
        resolved_source.registry_handle.as_deref(),
    )?;

    match crate::pkg::telemetry::posthog_capture_event(
        "exec",
        &pkg,
        env!("CARGO_PKG_VERSION"),
        resolved_source
            .registry_handle
            .as_deref()
            .unwrap_or("local"),
    ) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    println!("\n--- Executing '{}' ---\n", pkg.name.bold());

    let mut command_str = format!("\"{}\"", bin_path.display());
    if !args.is_empty() {
        command_str.push(' ');
        command_str.push_str(&args.join(" "));
    }

    println!("> {}", command_str.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("pwsh")
            .arg("-Command")
            .arg(&command_str)
            .status()?
    } else {
        Command::new("bash").arg("-c").arg(&command_str).status()?
    };

    if let Some(code) = status.code() {
        std::process::exit(code);
    }

    Ok(())
}
