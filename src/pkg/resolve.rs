use crate::pkg::{config, pin, types};
use chrono::Utc;
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SourceType {
    OfficialRepo,
    UntrustedRepo(String),
    GitRepo(String),
    LocalFile,
    Url,
}

#[derive(Debug)]
pub struct ResolvedSource {
    pub path: PathBuf,
    pub source_type: SourceType,
    pub repo_name: Option<String>,
    pub registry_handle: Option<String>,
    pub sharable_manifest: Option<types::SharableInstallManifest>,
}

#[derive(Debug, Default)]
struct PackageRequest {
    handle: Option<String>,
    repo: Option<String>,
    name: String,
    version_spec: Option<String>,
}

pub fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn parse_source_string(source_str: &str) -> Result<PackageRequest, Box<dyn Error>> {
    if source_str.contains('/')
        && (source_str.ends_with(".manifest.yaml") || source_str.ends_with(".pkg.lua"))
    {
        let path = std::path::Path::new(source_str);
        let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let name = if let Some(stripped) = file_stem.strip_suffix(".manifest") {
            stripped.to_string()
        } else if let Some(stripped) = file_stem.strip_suffix(".pkg") {
            stripped.to_string()
        } else {
            file_stem.to_string()
        };
        return Ok(PackageRequest {
            handle: None,
            repo: None,
            name,
            version_spec: None,
        });
    }

    let mut handle = None;
    let mut main_part = source_str;

    if main_part.starts_with('#') {
        if let Some(at_pos) = main_part.find('@') {
            if at_pos > 1 {
                handle = Some(main_part[1..at_pos].to_string());
                main_part = &main_part[at_pos..];
            } else {
                return Err("Invalid format: empty registry handle".into());
            }
        } else {
            return Err(
                "Invalid format: missing '@' after registry handle. Expected format: #handle@repo/package"
                    .into(),
            );
        }
    }

    let mut repo = None;
    let name: &str;
    let mut version_spec = None;

    let mut version_part_str = main_part;

    if let Some(at_pos) = main_part.rfind('@')
        && at_pos > 0
    {
        let (pkg_part, ver_part) = main_part.split_at(at_pos);
        version_part_str = pkg_part;
        version_spec = Some(ver_part[1..].to_string());
    }

    if version_part_str.starts_with('@') {
        let s = version_part_str.trim_start_matches('@');
        if let Some((repo_str, name_str)) = s.split_once('/') {
            if !name_str.is_empty() {
                repo = Some(repo_str.to_lowercase());
                name = name_str;
            } else {
                return Err("Invalid format: missing package name after repo path.".into());
            }
        } else {
            return Err(
                "Invalid format: must be in the form @repo/package or @repo/path/to/package".into(),
            );
        }
    } else {
        name = version_part_str;
    }

    if name.is_empty() {
        return Err("Invalid source string: package name is empty.".into());
    }

    Ok(PackageRequest {
        handle,
        repo,
        name: name.to_lowercase(),
        version_spec,
    })
}

fn find_package_in_db(request: &PackageRequest) -> Result<ResolvedSource, Box<dyn Error>> {
    let db_root = get_db_root()?;
    let config = config::read_config()?;

    let (registry_db_path, search_repos, is_default_registry, registry_handle) =
        if let Some(h) = &request.handle {
            let is_default = config
                .default_registry
                .as_ref()
                .is_some_and(|reg| reg.handle == *h);

            if is_default {
                let default_registry = config.default_registry.as_ref().unwrap();
                (
                    db_root.join(&default_registry.handle),
                    config.repos,
                    true,
                    Some(default_registry.handle.clone()),
                )
            } else if let Some(registry) = config.added_registries.iter().find(|r| r.handle == *h) {
                let repo_path = db_root.join(&registry.handle);
                let all_sub_repos = if repo_path.exists() {
                    fs::read_dir(&repo_path)?
                        .filter_map(Result::ok)
                        .filter(|entry| entry.path().is_dir() && entry.file_name() != ".git")
                        .map(|entry| entry.file_name().to_string_lossy().into_owned())
                        .collect()
                } else {
                    Vec::new()
                };
                (
                    repo_path,
                    all_sub_repos,
                    false,
                    Some(registry.handle.clone()),
                )
            } else {
                return Err(format!("Registry with handle '{}' not found.", h).into());
            }
        } else {
            let default_registry = config
                .default_registry
                .as_ref()
                .ok_or("No default registry set.")?;
            (
                db_root.join(&default_registry.handle),
                config.repos,
                true,
                Some(default_registry.handle.clone()),
            )
        };

    let repos_to_search = if let Some(r) = &request.repo {
        vec![r.clone()]
    } else {
        search_repos
    };

    struct FoundPackage {
        path: PathBuf,
        source_type: SourceType,
        repo_name: String,
        description: String,
    }

    let mut found_packages = Vec::new();

    if request.name.contains('/') {
        let pkg_name = Path::new(&request.name)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid package path: {}", request.name))?;

        for repo_name in &repos_to_search {
            let path = registry_db_path
                .join(repo_name)
                .join(&request.name)
                .join(format!("{}.pkg.lua", pkg_name));

            if path.exists() {
                let pkg: types::Package =
                    crate::pkg::lua::parser::parse_lua_package(path.to_str().unwrap(), None)?;
                let major_repo = repo_name.split('/').next().unwrap_or("").to_lowercase();

                let source_type = if is_default_registry {
                    let repo_config = config::read_repo_config(&registry_db_path).ok();
                    if let Some(ref cfg) = repo_config {
                        if let Some(repo_entry) = cfg.repos.iter().find(|r| r.name == major_repo) {
                            if repo_entry.repo_type == "offical" {
                                SourceType::OfficialRepo
                            } else {
                                SourceType::UntrustedRepo(repo_name.clone())
                            }
                        } else {
                            SourceType::UntrustedRepo(repo_name.clone())
                        }
                    } else {
                        SourceType::UntrustedRepo(repo_name.clone())
                    }
                } else {
                    SourceType::UntrustedRepo(repo_name.clone())
                };

                found_packages.push(FoundPackage {
                    path,
                    source_type,
                    repo_name: pkg.repo.clone(),
                    description: pkg.description,
                });
            }
        }
    } else {
        for repo_name in &repos_to_search {
            let repo_path = registry_db_path.join(repo_name);
            if !repo_path.is_dir() {
                continue;
            }
            for entry in WalkDir::new(&repo_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_dir() && e.file_name() == request.name.as_str())
            {
                let pkg_dir_path = entry.path();

                if let Ok(relative_path) = pkg_dir_path.strip_prefix(&repo_path) {
                    if relative_path.components().count() > 1 {
                        continue;
                    }
                } else {
                    continue;
                }

                let pkg_file_path = pkg_dir_path.join(format!("{}.pkg.lua", request.name));

                if pkg_file_path.exists() {
                    let pkg: types::Package = crate::pkg::lua::parser::parse_lua_package(
                        pkg_file_path.to_str().unwrap(),
                        None,
                    )?;
                    let major_repo = repo_name.split('/').next().unwrap_or("").to_lowercase();

                    let source_type = if is_default_registry {
                        let repo_config = config::read_repo_config(&registry_db_path).ok();
                        if let Some(ref cfg) = repo_config {
                            if let Some(repo_entry) =
                                cfg.repos.iter().find(|r| r.name == major_repo)
                            {
                                if repo_entry.repo_type == "offical" {
                                    SourceType::OfficialRepo
                                } else {
                                    SourceType::UntrustedRepo(repo_name.clone())
                                }
                            } else {
                                SourceType::UntrustedRepo(repo_name.clone())
                            }
                        } else {
                            SourceType::UntrustedRepo(repo_name.clone())
                        }
                    } else {
                        SourceType::UntrustedRepo(repo_name.clone())
                    };

                    found_packages.push(FoundPackage {
                        path: pkg_file_path,
                        source_type,
                        repo_name: pkg.repo.clone(),
                        description: pkg.description,
                    });
                }
            }
        }
    }

    if found_packages.is_empty() {
        if let Some(repo) = &request.repo {
            Err(format!(
                "Package '{}' not found in repository '@{}'.",
                request.name, repo
            )
            .into())
        } else {
            Err(format!(
                "Package '{}' not found in any active repositories.",
                request.name
            )
            .into())
        }
    } else if found_packages.len() == 1 {
        let chosen = &found_packages[0];

        Ok(ResolvedSource {
            path: chosen.path.clone(),
            source_type: chosen.source_type.clone(),
            repo_name: Some(chosen.repo_name.clone()),
            registry_handle: registry_handle.clone(),
            sharable_manifest: None,
        })
    } else {
        println!(
            "Found multiple packages named '{}'. Please choose one:",
            request.name.cyan()
        );

        let items: Vec<String> = found_packages
            .iter()
            .map(|p| format!("@{} - {}", p.repo_name.bold(), p.description))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a package")
            .items(&items)
            .default(0)
            .interact()?;

        let chosen = &found_packages[selection];
        println!(
            "Selected package '{}' from repo '{}'",
            request.name, chosen.repo_name
        );

        Ok(ResolvedSource {
            path: chosen.path.clone(),
            source_type: chosen.source_type.clone(),
            repo_name: Some(chosen.repo_name.clone()),
            registry_handle: registry_handle.clone(),
            sharable_manifest: None,
        })
    }
}

fn download_from_url(url: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    println!("Downloading package definition from URL...");
    let client = crate::utils::build_blocking_http_client(20)?;
    let mut attempt = 0u32;
    let mut response = loop {
        attempt += 1;
        match client.get(url).send() {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < 3 {
                    eprintln!(
                        "{}: download failed ({}). Retrying...",
                        "Network".yellow(),
                        e
                    );
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(format!(
                        "Failed to download file after {} attempts: {}",
                        attempt, e
                    )
                    .into());
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(format!(
            "Failed to download file (HTTP {}): {}",
            response.status(),
            url
        )
        .into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

    let mut downloaded_bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message("Download complete.");

    let content = String::from_utf8(downloaded_bytes)?;

    let temp_path = env::temp_dir().join(format!(
        "zoi-temp-{}.pkg.lua",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    fs::write(&temp_path, content)?;

    Ok(ResolvedSource {
        path: temp_path,
        source_type: SourceType::Url,
        repo_name: None,
        registry_handle: Some("local".to_string()),
        sharable_manifest: None,
    })
}

fn download_content_from_url(url: &str) -> Result<String, Box<dyn Error>> {
    println!("Downloading from: {}", url.cyan());
    let client = crate::utils::build_blocking_http_client(20)?;
    let mut attempt = 0u32;
    let response = loop {
        attempt += 1;
        match client.get(url).send() {
            Ok(resp) => break resp,
            Err(e) => {
                if attempt < 3 {
                    eprintln!(
                        "{}: download failed ({}). Retrying...",
                        "Network".yellow(),
                        e
                    );
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(format!(
                        "Failed to download from {} after {} attempts: {}",
                        url, attempt, e
                    )
                    .into());
                }
            }
        }
    };

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download from {} (HTTP {}). Content: {}",
            url,
            response.status(),
            response
                .text()
                .unwrap_or_else(|_| "Could not read response body".to_string())
        )
        .into());
    }

    Ok(response.text()?)
}

fn resolve_version_from_url(url: &str, channel: &str) -> Result<String, Box<dyn Error>> {
    println!(
        "Resolving version for channel '{}' from {}",
        channel.cyan(),
        url.cyan()
    );
    let client = crate::utils::build_blocking_http_client(15)?;
    let mut attempt = 0u32;
    let resp = loop {
        attempt += 1;
        match client.get(url).send() {
            Ok(r) => match r.text() {
                Ok(t) => break t,
                Err(e) => {
                    if attempt < 3 {
                        eprintln!("{}: read failed ({}). Retrying...", "Network".yellow(), e);
                        crate::utils::retry_backoff_sleep(attempt);
                        continue;
                    } else {
                        return Err(format!(
                            "Failed to read response after {} attempts: {}",
                            attempt, e
                        )
                        .into());
                    }
                }
            },
            Err(e) => {
                if attempt < 3 {
                    eprintln!("{}: fetch failed ({}). Retrying...", "Network".yellow(), e);
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(format!("Failed to fetch after {} attempts: {}", attempt, e).into());
                }
            }
        }
    };
    let json: serde_json::Value = serde_json::from_str(&resp)?;

    if let Some(version) = json
        .get("versions")
        .and_then(|v| v.get(channel))
        .and_then(|c| c.as_str())
    {
        return Ok(version.to_string());
    }

    Err(format!("Failed to extract version for channel '{channel}' from JSON URL: {url}").into())
}

fn resolve_channel(
    versions: &HashMap<String, String>,
    channel: &str,
) -> Result<String, Box<dyn Error>> {
    if let Some(url_or_version) = versions.get(channel) {
        if url_or_version.starts_with("http") {
            resolve_version_from_url(url_or_version, channel)
        } else {
            Ok(url_or_version.clone())
        }
    } else {
        Err(format!("Channel '@{}' not found in versions map.", channel).into())
    }
}

pub fn get_default_version(
    pkg: &types::Package,
    registry_handle: Option<&str>,
) -> Result<String, Box<dyn Error>> {
    if let Some(handle) = registry_handle {
        let source = format!("#{}@{}", handle, pkg.repo);

        if let Some(pinned_version) = pin::get_pinned_version(&source)? {
            println!(
                "Using pinned version '{}' for {}.",
                pinned_version.yellow(),
                source.cyan()
            );
            return if pinned_version.starts_with('@') {
                let channel = pinned_version.trim_start_matches('@');
                let versions = pkg.versions.as_ref().ok_or_else(|| {
                    format!(
                        "Package '{}' has no 'versions' map to resolve pinned channel '{}'.",
                        pkg.name, pinned_version
                    )
                })?;
                resolve_channel(versions, channel)
            } else {
                Ok(pinned_version)
            };
        }
    }

    if let Some(versions) = &pkg.versions {
        if versions.contains_key("stable") {
            return resolve_channel(versions, "stable");
        }
        if let Some((channel, _)) = versions.iter().next() {
            println!(
                "No 'stable' channel found, using first available channel: '@{}'",
                channel.cyan()
            );
            return resolve_channel(versions, channel);
        }
        return Err("Package has a 'versions' map but no versions were found in it.".into());
    }

    if let Some(ver) = &pkg.version {
        if ver.starts_with("http") {
            let client = crate::utils::build_blocking_http_client(15)?;
            let mut attempt = 0u32;
            let resp = loop {
                attempt += 1;
                match client.get(ver).send() {
                    Ok(r) => match r.text() {
                        Ok(t) => break t,
                        Err(e) => {
                            if attempt < 3 {
                                eprintln!(
                                    "{}: read failed ({}). Retrying...",
                                    "Network".yellow(),
                                    e
                                );
                                crate::utils::retry_backoff_sleep(attempt);
                                continue;
                            } else {
                                return Err(format!(
                                    "Failed to read response after {} attempts: {}",
                                    attempt, e
                                )
                                .into());
                            }
                        }
                    },
                    Err(e) => {
                        if attempt < 3 {
                            eprintln!("{}: fetch failed ({}). Retrying...", "Network".yellow(), e);
                            crate::utils::retry_backoff_sleep(attempt);
                            continue;
                        } else {
                            return Err(format!(
                                "Failed to fetch after {} attempts: {}",
                                attempt, e
                            )
                            .into());
                        }
                    }
                }
            };
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp) {
                if let Some(version) = json
                    .get("versions")
                    .and_then(|v| v.get("stable"))
                    .and_then(|s| s.as_str())
                {
                    return Ok(version.to_string());
                }

                if let Some(tag) = json
                    .get("latest")
                    .and_then(|l| l.get("production"))
                    .and_then(|p| p.get("tag"))
                    .and_then(|t| t.as_str())
                {
                    return Ok(tag.to_string());
                }
                return Err(format!(
                    "Could not determine a version from the JSON content at {}",
                    ver
                )
                .into());
            }
            return Ok(resp.trim().to_string());
        } else {
            return Ok(ver.clone());
        }
    }

    Err(format!("Could not determine a version for package '{}'.", pkg.name).into())
}

fn get_version_for_install(
    pkg: &types::Package,
    version_spec: &Option<String>,
    registry_handle: Option<&str>,
) -> Result<String, Box<dyn Error>> {
    if let Some(spec) = version_spec {
        if spec.starts_with('@') {
            let channel = spec.trim_start_matches('@');
            let versions = pkg.versions.as_ref().ok_or_else(|| {
                format!(
                    "Package '{}' has no 'versions' map to resolve channel '@{}'.",
                    pkg.name, channel
                )
            })?;
            return resolve_channel(versions, channel);
        }

        if let Some(versions) = &pkg.versions
            && versions.contains_key(spec)
        {
            println!("Found '{}' as a channel, resolving...", spec.cyan());
            return resolve_channel(versions, spec);
        }

        return Ok(spec.clone());
    }

    get_default_version(pkg, registry_handle)
}

pub fn resolve_source(source: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    let resolved = resolve_source_recursive(source, 0)?;

    if let Ok(request) = parse_source_string(source)
        && !matches!(
            &resolved.source_type,
            SourceType::LocalFile | SourceType::Url
        )
        && let Some(repo_name) = &resolved.repo_name
    {
        println!("Found package '{}' in repo '{}'", request.name, repo_name);
    }

    Ok(resolved)
}

pub fn resolve_package_and_version(
    source_str: &str,
) -> Result<
    (
        types::Package,
        String,
        Option<types::SharableInstallManifest>,
        PathBuf,
        Option<String>,
    ),
    Box<dyn Error>,
> {
    let request = parse_source_string(source_str)?;
    let resolved_source = resolve_source_recursive(source_str, 0)?;
    let registry_handle = resolved_source.registry_handle.clone();
    let pkg_lua_path = resolved_source.path.clone();

    let pkg_template =
        crate::pkg::lua::parser::parse_lua_package(resolved_source.path.to_str().unwrap(), None)?;

    let mut pkg_with_repo = pkg_template;
    if let Some(repo_name) = resolved_source.repo_name.clone() {
        pkg_with_repo.repo = repo_name;
    }

    let version_string = get_version_for_install(
        &pkg_with_repo,
        &request.version_spec,
        registry_handle.as_deref(),
    )?;

    let mut pkg = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().unwrap(),
        Some(&version_string),
    )?;
    if let Some(repo_name) = resolved_source.repo_name.clone() {
        pkg.repo = repo_name;
    }
    pkg.version = Some(version_string.clone());

    let registry_handle = resolved_source.registry_handle.clone();

    Ok((
        pkg,
        version_string,
        resolved_source.sharable_manifest,
        pkg_lua_path,
        registry_handle,
    ))
}

fn resolve_source_recursive(source: &str, depth: u8) -> Result<ResolvedSource, Box<dyn Error>> {
    if depth > 5 {
        return Err("Exceeded max resolution depth, possible circular 'alt' reference.".into());
    }

    if source.ends_with(".manifest.yaml") {
        let path = PathBuf::from(source);
        if !path.exists() {
            return Err(format!("Local file not found at '{source}'").into());
        }
        println!("Using local sharable manifest file: {}", path.display());
        let content = fs::read_to_string(&path)?;
        let sharable_manifest: types::SharableInstallManifest = serde_yaml::from_str(&content)?;
        let new_source = format!(
            "#{}@{}/{}@{}",
            sharable_manifest.registry_handle,
            sharable_manifest.repo,
            sharable_manifest.name,
            sharable_manifest.version
        );
        let mut resolved_source = resolve_source_recursive(&new_source, depth + 1)?;
        resolved_source.sharable_manifest = Some(sharable_manifest);
        return Ok(resolved_source);
    }

    let request = parse_source_string(source)?;

    if let Some(handle) = &request.handle
        && handle.starts_with("git:")
    {
        let git_source = handle.strip_prefix("git:").unwrap();
        println!(
            "Warning: using remote git repo '{}' not from official Zoi database.",
            git_source.yellow()
        );

        let (host, repo_path) = git_source
            .split_once('/')
            .ok_or("Invalid git source format. Expected host/owner/repo.")?;

        let (base_url, branch_sep) = match host {
            "github.com" => (
                format!("https://raw.githubusercontent.com/{}", repo_path),
                "/",
            ),
            "gitlab.com" => (format!("https://gitlab.com/{}/-/raw", repo_path), "/"),
            "codeberg.org" => (
                format!("https://codeberg.org/{}/raw/branch", repo_path),
                "/",
            ),
            _ => return Err(format!("Unsupported git host: {}", host).into()),
        };

        let (_, branch) = {
            let mut last_error = None;
            let mut content = None;
            for b in ["main", "master"] {
                let repo_yaml_url = format!("{}{}{}/repo.yaml", base_url, branch_sep, b);
                match download_content_from_url(&repo_yaml_url) {
                    Ok(c) => {
                        content = Some((c, b.to_string()));
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e);
                    }
                }
            }
            content
                .ok_or(last_error.unwrap_or_else(|| {
                    "Could not find repo.yaml on main or master branch".into()
                }))?
        };

        let full_pkg_path = if let Some(r) = &request.repo {
            format!("{}/{}", r, request.name)
        } else {
            request.name.clone()
        };

        let pkg_name = Path::new(&full_pkg_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let pkg_lua_filename = format!("{}.pkg.lua", pkg_name);
        let pkg_lua_path_in_repo = Path::new(&full_pkg_path).join(pkg_lua_filename);

        let pkg_lua_url = format!(
            "{}{}{}/{}",
            base_url,
            branch_sep,
            branch,
            pkg_lua_path_in_repo.to_str().unwrap().replace('\\', "/")
        );

        let pkg_lua_content = download_content_from_url(&pkg_lua_url)?;

        let temp_path = env::temp_dir().join(format!(
            "zoi-temp-git-{}.pkg.lua",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::write(&temp_path, pkg_lua_content)?;

        let repo_name = format!("git:{}", git_source);

        return Ok(ResolvedSource {
            path: temp_path,
            source_type: SourceType::GitRepo(repo_name.clone()),
            repo_name: Some(repo_name),
            registry_handle: None,
            sharable_manifest: None,
        });
    }

    let resolved_source = if source.starts_with("@git/") {
        let full_path_str = source.trim_start_matches("@git/");
        let parts: Vec<&str> = full_path_str.split('/').collect();

        if parts.len() < 2 {
            return Err("Invalid git source. Use @git/<repo-name>/<path/to/pkg>".into());
        }

        let repo_name = parts[0];
        let nested_path_parts = &parts[1..];
        let pkg_name = nested_path_parts.last().unwrap();

        let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
        let mut path = home_dir
            .join(".zoi")
            .join("pkgs")
            .join("git")
            .join(repo_name);

        for part in nested_path_parts.iter().take(nested_path_parts.len() - 1) {
            path = path.join(part);
        }

        path = path.join(format!("{}.pkg.lua", pkg_name));

        if !path.exists() {
            let nested_path_str = nested_path_parts.join("/");
            return Err(format!(
                "Package '{}' not found in git repo '{}' (expected: {})",
                nested_path_str,
                repo_name,
                path.display()
            )
            .into());
        }
        println!(
            "Warning: using external git repo '{}{}' not from official Zoi database.",
            "@git/".yellow(),
            repo_name.yellow()
        );
        ResolvedSource {
            path,
            source_type: SourceType::GitRepo(repo_name.to_string()),
            repo_name: Some(format!("git/{}", repo_name)),
            registry_handle: Some("local".to_string()),
            sharable_manifest: None,
        }
    } else if source.starts_with("http://") || source.starts_with("https://") {
        download_from_url(source)?
    } else if source.ends_with(".pkg.lua") {
        let path = PathBuf::from(source);
        if !path.exists() {
            return Err(format!("Local file not found at '{source}'").into());
        }
        println!("Using local package file: {}", path.display());
        ResolvedSource {
            path,
            source_type: SourceType::LocalFile,
            repo_name: None,
            registry_handle: Some("local".to_string()),
            sharable_manifest: None,
        }
    } else {
        find_package_in_db(&request)?
    };

    let pkg_for_alt_check =
        crate::pkg::lua::parser::parse_lua_package(resolved_source.path.to_str().unwrap(), None)?;

    if let Some(alt_source) = pkg_for_alt_check.alt {
        println!("Found 'alt' source. Resolving from: {}", alt_source.cyan());

        let mut alt_resolved_source =
            if alt_source.starts_with("http://") || alt_source.starts_with("https://") {
                println!("Downloading 'alt' source from: {}", alt_source.cyan());
                let client = crate::utils::build_blocking_http_client(20)?;
                let mut attempt = 0u32;
                let response = loop {
                    attempt += 1;
                    match client.get(&alt_source).send() {
                        Ok(resp) => break resp,
                        Err(e) => {
                            if attempt < 3 {
                                eprintln!(
                                    "{}: download failed ({}). Retrying...",
                                    "Network".yellow(),
                                    e
                                );
                                crate::utils::retry_backoff_sleep(attempt);
                                continue;
                            } else {
                                return Err(format!(
                                    "Failed to download file after {} attempts: {}",
                                    attempt, e
                                )
                                .into());
                            }
                        }
                    }
                };
                if !response.status().is_success() {
                    return Err(format!(
                        "Failed to download alt source (HTTP {}): {}",
                        response.status(),
                        alt_source
                    )
                    .into());
                }

                let content = response.text()?;
                let temp_path = env::temp_dir().join(format!(
                    "zoi-alt-{}.pkg.lua",
                    Utc::now().timestamp_nanos_opt().unwrap_or(0)
                ));
                fs::write(&temp_path, &content)?;
                resolve_source_recursive(temp_path.to_str().unwrap(), depth + 1)?
            } else {
                resolve_source_recursive(&alt_source, depth + 1)?
            };

        if resolved_source.source_type == SourceType::OfficialRepo {
            alt_resolved_source.source_type = SourceType::OfficialRepo;
        }

        return Ok(alt_resolved_source);
    }

    Ok(resolved_source)
}
