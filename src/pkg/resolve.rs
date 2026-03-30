use crate::pkg::{config, pin, types};
use anyhow::{Result, anyhow};
use colored::*;
use comfy_table::{Table, presets::UTF8_FULL};
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::collections::HashMap;
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
    pub git_sha: Option<String>,
}

#[derive(Debug, Default)]
pub struct PackageRequest {
    pub handle: Option<String>,
    pub repo: Option<String>,
    pub name: String,
    pub sub_package: Option<String>,
    pub version_spec: Option<String>,
}

use std::sync::LazyLock;
use std::sync::Mutex;

static HANDLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:#(?P<handle>[^@]+))?(?P<main_part>.*)$")
        .expect("Static HANDLE_RE regex is valid")
});
static MAIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@?(?P<repo_and_name>[^@]+)(?:@(?P<version>.+))?$")
        .expect("Static MAIN_RE regex is valid")
});
static CONFIRMED_UNTRUSTED_SOURCES: LazyLock<Mutex<std::collections::HashSet<String>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

fn split_explicit_file_source(source_str: &str) -> Option<(&str, Option<String>, Option<String>)> {
    let (main_part, version_spec) = if let Some((base, version)) = source_str.rsplit_once('@') {
        let base_path = if let Some((path, sub)) = base.rsplit_once(':') {
            if (path.ends_with(".pkg.lua") || path.ends_with(".manifest.yaml"))
                && !sub.contains('/')
            {
                path
            } else {
                base
            }
        } else {
            base
        };

        if base_path.ends_with(".pkg.lua") || base_path.ends_with(".manifest.yaml") {
            (base, Some(version.to_string()))
        } else {
            (source_str, None)
        }
    } else {
        (source_str, None)
    };

    let (path_part, sub_package) = if let Some((base, sub)) = main_part.rsplit_once(':') {
        if (base.ends_with(".pkg.lua") || base.ends_with(".manifest.yaml")) && !sub.contains('/') {
            (base, Some(sub.to_string()))
        } else {
            (main_part, None)
        }
    } else {
        (main_part, None)
    };

    if path_part.ends_with(".pkg.lua") || path_part.ends_with(".manifest.yaml") {
        Some((path_part, sub_package, version_spec))
    } else {
        None
    }
}

fn download_source_for_explicit_path<'a>(source: &'a str, path_part: Option<&'a str>) -> &'a str {
    path_part.unwrap_or(source)
}

fn get_git_head_sha(repo_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let head = repo.head().ok()?;
    let target = head.target()?;
    Some(target.to_string())
}

pub fn get_db_root() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("ZOI_DB_DIR") {
        return Ok(PathBuf::from(path));
    }
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    Ok(crate::pkg::sysroot::apply_sysroot(
        home_dir.join(".zoi").join("pkgs").join("db"),
    ))
}

pub fn parse_source_string(source_str: &str) -> Result<PackageRequest> {
    if let Some((path_part, sub_package_from_path, version_spec)) =
        split_explicit_file_source(source_str)
    {
        let path = std::path::Path::new(path_part);
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
            sub_package: sub_package_from_path,
            version_spec,
        });
    }

    let caps = HANDLE_RE
        .captures(source_str)
        .ok_or_else(|| anyhow!("Invalid source string format"))?;
    let handle = caps.name("handle").map(|m| m.as_str().to_string());
    let main_part = caps
        .name("main_part")
        .expect("Regex matched but main_part group not found")
        .as_str();

    let caps_main = MAIN_RE
        .captures(main_part)
        .ok_or_else(|| anyhow!("Invalid source string format"))?;

    let repo_and_name = caps_main
        .name("repo_and_name")
        .expect("Regex matched but repo_and_name group not found")
        .as_str();
    let version_spec = caps_main.name("version").map(|m| m.as_str().to_string());

    let (repo, name_and_sub) = if main_part.starts_with('@') {
        if let Some(slash_pos) = repo_and_name.find('/') {
            let (repo_str, name_str) = repo_and_name.split_at(slash_pos);
            (Some(repo_str.to_lowercase()), &name_str[1..])
        } else {
            return Err(anyhow!("Invalid repo format: expected @repo/name"));
        }
    } else {
        (None, repo_and_name)
    };

    let (name, sub_package) = if let Some((n, s)) = name_and_sub.rsplit_once(':') {
        (n, Some(s.to_string()))
    } else {
        (name_and_sub, None)
    };

    if name.is_empty() {
        return Err(anyhow!("Invalid source string: package name is empty."));
    }

    Ok(PackageRequest {
        handle,
        repo,
        name: name.to_lowercase(),
        sub_package,
        version_spec,
    })
}

fn find_package_in_db(request: &PackageRequest, quiet: bool) -> Result<ResolvedSource> {
    let db_root = get_db_root()?;
    let config = config::read_config()?;

    let (registry_db_path, search_repos, is_default_registry, registry_handle) =
        if let Some(h) = &request.handle {
            let is_default = config
                .default_registry
                .as_ref()
                .is_some_and(|reg| reg.handle == *h);

            if is_default {
                let default_registry = config
                    .default_registry
                    .as_ref()
                    .ok_or_else(|| anyhow!("Default registry not found"))?;
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
                return Err(anyhow!("Registry with handle '{}' not found.", h));
            }
        } else {
            let default_registry = config
                .default_registry
                .as_ref()
                .ok_or_else(|| anyhow!("No default registry set."))?;
            (
                db_root.join(&default_registry.handle),
                config.repos,
                true,
                Some(default_registry.handle.clone()),
            )
        };

    if !registry_db_path.exists() {
        return Err(anyhow!(
            "Registry '{}' is not synced. Please run 'zoi sync' to download the package database.",
            registry_handle.unwrap_or_else(|| "default".to_string())
        ));
    }

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
        license: String,
        size: Option<u64>,
    }

    fn process_found_package(
        path: PathBuf,
        repo_name: &str,
        is_default_registry: bool,
        registry_db_path: &Path,
        quiet: bool,
    ) -> Result<FoundPackage> {
        let pkg: types::Package = crate::pkg::lua::parser::parse_lua_package(
            path.to_str()
                .ok_or_else(|| anyhow!("Path contains invalid UTF-8 characters: {:?}", path))?,
            None,
            quiet,
        )?;
        let major_repo = repo_name
            .split('/')
            .next()
            .unwrap_or_default()
            .to_lowercase();

        let source_type = if is_default_registry {
            let repo_config = config::read_repo_config(registry_db_path).ok();
            if let Some(ref cfg) = repo_config {
                if let Some(repo_entry) = cfg.repos.iter().find(|r| r.name == major_repo) {
                    if repo_entry.repo_type == "official" {
                        SourceType::OfficialRepo
                    } else {
                        SourceType::UntrustedRepo(repo_name.to_string())
                    }
                } else {
                    SourceType::UntrustedRepo(repo_name.to_string())
                }
            } else {
                SourceType::UntrustedRepo(repo_name.to_string())
            }
        } else {
            SourceType::UntrustedRepo(repo_name.to_string())
        };

        Ok(FoundPackage {
            path,
            source_type,
            repo_name: pkg.repo.clone(),
            description: pkg.description,
            license: pkg.license,
            size: pkg.installed_size,
        })
    }

    let mut found_packages = Vec::new();

    if request.name.contains('/') {
        let pkg_name = Path::new(&request.name)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid package path: {}", request.name))?;

        for repo_name in &repos_to_search {
            let path = registry_db_path
                .join(repo_name)
                .join(&request.name)
                .join(format!("{}.pkg.lua", pkg_name));

            if path.exists()
                && let Ok(found) = process_found_package(
                    path,
                    repo_name,
                    is_default_registry,
                    &registry_db_path,
                    quiet,
                )
            {
                found_packages.push(found);
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

                if pkg_file_path.exists()
                    && let Ok(found) = process_found_package(
                        pkg_file_path,
                        repo_name,
                        is_default_registry,
                        &registry_db_path,
                        quiet,
                    )
                {
                    found_packages.push(found);
                }
            }
        }
    }

    if found_packages.is_empty() {
        for repo_name in &repos_to_search {
            let repo_path = registry_db_path.join(repo_name);
            if !repo_path.is_dir() {
                continue;
            }
            for entry in WalkDir::new(&repo_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".pkg.lua")
                })
            {
                if let Ok(pkg) = crate::pkg::lua::parser::parse_lua_package(
                    entry.path().to_str().ok_or_else(|| {
                        anyhow!("Path contains invalid UTF-8 characters: {:?}", entry.path())
                    })?,
                    None,
                    true,
                ) && let Some(provides) = &pkg.provides
                    && provides.iter().any(|p| p == &request.name)
                {
                    let major_repo = repo_name
                        .split('/')
                        .next()
                        .unwrap_or_default()
                        .to_lowercase();
                    let source_type = if is_default_registry {
                        let repo_config = config::read_repo_config(&registry_db_path).ok();
                        if let Some(ref cfg) = repo_config {
                            if let Some(repo_entry) =
                                cfg.repos.iter().find(|r| r.name == major_repo)
                            {
                                if repo_entry.repo_type == "official" {
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
                        path: entry.path().to_path_buf(),
                        source_type,
                        repo_name: pkg.repo.clone(),
                        description: pkg.description,
                        license: pkg.license,
                        size: pkg.installed_size,
                    });
                }
            }
        }
    }

    if found_packages.is_empty() {
        if let Some(repo) = &request.repo {
            Err(anyhow!(
                "Package '{}' not found in repository '@{}'.",
                request.name,
                repo
            ))
        } else {
            Err(anyhow!(
                "Package '{}' not found in any active repositories.",
                request.name
            ))
        }
    } else if found_packages.len() == 1 {
        let chosen = &found_packages[0];

        Ok(ResolvedSource {
            path: chosen.path.clone(),
            source_type: chosen.source_type.clone(),
            repo_name: Some(chosen.repo_name.clone()),
            registry_handle: registry_handle.clone(),
            sharable_manifest: None,
            git_sha: None,
        })
    } else {
        println!(
            "Found multiple packages named or providing '{}'. Please choose one:",
            request.name.cyan()
        );

        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["#", "Repo", "License", "Size", "Description"]);

        for (i, p) in found_packages.iter().enumerate() {
            table.add_row(vec![
                (i + 1).to_string(),
                p.repo_name.clone(),
                p.license.clone(),
                p.size
                    .map(crate::utils::format_bytes)
                    .unwrap_or_else(|| "unknown".to_string()),
                p.description.clone(),
            ]);
        }
        println!("{table}");

        let items: Vec<String> = found_packages
            .iter()
            .map(|p| format!("@{}", p.repo_name.bold()))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a provider")
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
            git_sha: None,
        })
    }
}

fn download_from_url(url: &str) -> Result<ResolvedSource> {
    println!("Downloading package definition from URL...");
    let client = crate::utils::get_http_client()?;
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
                    return Err(anyhow!(
                        "Failed to download file after {} attempts: {}",
                        attempt,
                        e
                    ));
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download file (HTTP {}): {}",
            response.status(),
            url
        ));
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

    let mut temp_file = tempfile::Builder::new()
        .prefix("zoi-temp-")
        .suffix(".pkg.lua")
        .tempfile()?;
    use std::io::Write;
    temp_file.write_all(&downloaded_bytes)?;
    let temp_path = temp_file.into_temp_path();

    Ok(ResolvedSource {
        path: temp_path.to_path_buf(),
        source_type: SourceType::Url,
        repo_name: None,
        registry_handle: Some("local".to_string()),
        sharable_manifest: None,
        git_sha: None,
    })
}

fn download_content_from_url(url: &str) -> Result<String> {
    println!("Downloading from: {}", url.cyan());
    let client = crate::utils::get_http_client()?;
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
                    return Err(anyhow!(
                        "Failed to download from {} after {} attempts: {}",
                        url,
                        attempt,
                        e
                    ));
                }
            }
        }
    };

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download from {} (HTTP {}). Content: {}",
            url,
            response.status(),
            response
                .text()
                .unwrap_or_else(|_| "Could not read response body".to_string())
        ));
    }

    Ok(response.text()?)
}

pub fn resolve_version_from_url(url: &str, channel: &str) -> Result<String> {
    println!(
        "Resolving version for channel '{}' from {}",
        channel.cyan(),
        url.cyan()
    );
    let client = crate::utils::get_http_client()?;
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
                        return Err(anyhow!(
                            "Failed to read response after {} attempts: {}",
                            attempt,
                            e
                        ));
                    }
                }
            },
            Err(e) => {
                if attempt < 3 {
                    eprintln!("{}: fetch failed ({}). Retrying...", "Network".yellow(), e);
                    crate::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(anyhow!("Failed to fetch after {} attempts: {}", attempt, e));
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

    Err(anyhow!(
        "Failed to extract version for channel '{channel}' from JSON URL: {url}"
    ))
}

pub fn resolve_channel(versions: &HashMap<String, String>, channel: &str) -> Result<String> {
    if let Some(url_or_version) = versions.get(channel) {
        if url_or_version.starts_with("http") {
            resolve_version_from_url(url_or_version, channel)
        } else {
            Ok(url_or_version.clone())
        }
    } else {
        Err(anyhow!("Channel '@{}' not found in versions map.", channel))
    }
}

pub fn get_default_version(pkg: &types::Package, registry_handle: Option<&str>) -> Result<String> {
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
                    anyhow!(
                        "Package '{}' has no 'versions' map to resolve pinned channel '{}'.",
                        pkg.name,
                        pinned_version
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
        let mut channels: Vec<_> = versions.keys().collect();
        channels.sort();
        if let Some(channel) = channels.first() {
            println!(
                "No 'stable' channel found, using first available channel: '@{}'",
                channel.cyan()
            );
            return resolve_channel(versions, channel);
        }
        return Err(anyhow!(
            "Package has a 'versions' map but no versions were found in it."
        ));
    }

    if let Some(ver) = &pkg.version {
        if ver.starts_with("http") {
            let client = crate::utils::get_http_client()?;
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
                                return Err(anyhow!(
                                    "Failed to read response after {} attempts: {}",
                                    attempt,
                                    e
                                ));
                            }
                        }
                    },
                    Err(e) => {
                        if attempt < 3 {
                            eprintln!("{}: fetch failed ({}). Retrying...", "Network".yellow(), e);
                            crate::utils::retry_backoff_sleep(attempt);
                            continue;
                        } else {
                            return Err(anyhow!(
                                "Failed to fetch after {} attempts: {}",
                                attempt,
                                e
                            ));
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
                return Err(anyhow!(
                    "Could not determine a version from the JSON content at {}",
                    ver
                ));
            }
            return Ok(resp.trim().to_string());
        } else {
            return Ok(ver.clone());
        }
    }

    Err(anyhow!(
        "Could not determine a version for package '{}'.",
        pkg.name
    ))
}

fn get_version_for_install(
    pkg: &types::Package,
    version_spec: &Option<String>,
    registry_handle: Option<&str>,
) -> Result<String> {
    if let Some(spec) = version_spec {
        if spec.starts_with('@') {
            let channel = spec.trim_start_matches('@');
            let versions = pkg.versions.as_ref().ok_or_else(|| {
                anyhow!(
                    "Package '{}' has no 'versions' map to resolve channel '@{}'.",
                    pkg.name,
                    channel
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

pub fn resolve_requested_version_spec(
    source_str: &str,
    quiet: bool,
    yes: bool,
) -> Result<Option<String>> {
    let request = parse_source_string(source_str)?;
    let Some(_) = request.version_spec else {
        return Ok(None);
    };

    let resolved_source = resolve_source(source_str, quiet, yes)?;
    let mut pkg = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {:?}",
                resolved_source.path
            )
        })?,
        None,
        quiet,
    )?;

    if let Some(repo_name) = resolved_source.repo_name {
        pkg.repo = repo_name;
    }

    get_version_for_install(
        &pkg,
        &request.version_spec,
        resolved_source.registry_handle.as_deref(),
    )
    .map(Some)
}

pub fn resolve_source(source: &str, quiet: bool, yes: bool) -> Result<ResolvedSource> {
    let config = config::read_config().unwrap_or_default();
    let max_depth = config.max_resolution_depth.unwrap_or(7);
    let resolved = resolve_source_recursive(source, 0, max_depth, quiet)?;

    if !quiet {
        let confirmation_key = match &resolved.source_type {
            SourceType::LocalFile => Some(
                resolved
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| resolved.path.clone())
                    .to_string_lossy()
                    .to_string(),
            ),
            SourceType::Url => Some(source.to_string()),
            _ => None,
        };

        let confirmation_key = if let Some(key) = confirmation_key {
            let confirmed = CONFIRMED_UNTRUSTED_SOURCES
                .lock()
                .map_err(|e| anyhow!("Failed to lock trust confirmation cache: {}", e))?;
            if confirmed.contains(&key) {
                None
            } else {
                Some(key)
            }
        } else {
            None
        };

        if let Some(key) = confirmation_key {
            crate::utils::confirm_untrusted_source(&resolved.source_type, yes)?;
            let mut confirmed = CONFIRMED_UNTRUSTED_SOURCES
                .lock()
                .map_err(|e| anyhow!("Failed to lock trust confirmation cache: {}", e))?;
            confirmed.insert(key);
        }
    }

    if let Ok(_request) = parse_source_string(source)
        && !matches!(
            &resolved.source_type,
            SourceType::LocalFile | SourceType::Url
        )
        && let Some(_repo_name) = &resolved.repo_name
    {}

    Ok(resolved)
}

pub fn resolve_package_and_version(
    source_str: &str,
    quiet: bool,
    yes: bool,
) -> Result<(
    types::Package,
    String,
    Option<types::SharableInstallManifest>,
    PathBuf,
    Option<String>,
    Option<String>,
)> {
    let request = parse_source_string(source_str)?;
    let resolved_source = resolve_source(source_str, quiet, yes)?;
    let registry_handle = resolved_source.registry_handle.clone();
    let pkg_lua_path = resolved_source.path.clone();
    let git_sha = resolved_source.git_sha.clone();

    let pkg_template = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {:?}",
                resolved_source.path
            )
        })?,
        None,
        quiet,
    )?;

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
        resolved_source.path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {:?}",
                resolved_source.path
            )
        })?,
        Some(&version_string),
        quiet,
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
        git_sha,
    ))
}

fn resolve_source_recursive(
    source: &str,
    depth: u8,
    max_depth: u8,
    quiet: bool,
) -> Result<ResolvedSource> {
    if max_depth > 0 && depth > max_depth {
        let msg = format!(
            "Resolution depth {} exceeds limit {}. Potential circular 'alt' reference.",
            depth, max_depth
        );
        if quiet || !crate::utils::ask_for_confirmation(&format!("{} Continue anyway?", msg), false)
        {
            return Err(anyhow!("Exceeded max resolution depth."));
        }
    }

    if source.ends_with(".manifest.yaml") {
        let path = PathBuf::from(source);
        if !path.exists() {
            return Err(anyhow!("Local file not found at '{source}'"));
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
        let mut resolved_source =
            resolve_source_recursive(&new_source, depth + 1, max_depth, quiet)?;
        resolved_source.sharable_manifest = Some(sharable_manifest);
        return Ok(resolved_source);
    }

    let path_part = split_explicit_file_source(source).map(|(path, _, _)| path);

    let request = parse_source_string(source)?;

    if let Some(handle) = &request.handle
        && handle.starts_with("git:")
    {
        if crate::pkg::offline::is_offline() {
            return Err(anyhow!(
                "Cannot resolve remote git repo '{}': Zoi is in offline mode.",
                handle
            ));
        }
        let git_source = handle
            .strip_prefix("git:")
            .expect("Handle was checked to start with git: above");
        println!(
            "Warning: using remote git repo '{}' not from official Zoi database.",
            git_source.yellow()
        );

        let (host, repo_path) = git_source
            .split_once('/')
            .ok_or_else(|| anyhow!("Invalid git source format. Expected host/owner/repo."))?;

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
            _ => return Err(anyhow!("Unsupported git host: {}", host)),
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
            content.ok_or_else(|| {
                last_error
                    .unwrap_or_else(|| anyhow!("Could not find repo.yaml on main or master branch"))
            })?
        };

        let full_pkg_path = if let Some(r) = &request.repo {
            format!("{}/{}", r, request.name)
        } else {
            request.name.clone()
        };

        let pkg_name = Path::new(&full_pkg_path)
            .file_name()
            .ok_or_else(|| anyhow!("Invalid package path: {}", full_pkg_path))?
            .to_str()
            .ok_or_else(|| anyhow!("Package name contains invalid UTF-8: {}", full_pkg_path))?;
        let pkg_lua_filename = format!("{}.pkg.lua", pkg_name);
        let pkg_lua_path_in_repo = Path::new(&full_pkg_path).join(pkg_lua_filename);

        let pkg_lua_url = format!(
            "{}{}{}/{}",
            base_url,
            branch_sep,
            branch,
            pkg_lua_path_in_repo
                .to_str()
                .ok_or_else(|| anyhow!("Package path contains invalid UTF-8"))?
                .replace('\\', "/")
        );

        let pkg_lua_content = download_content_from_url(&pkg_lua_url)?;

        let mut temp_file = tempfile::Builder::new()
            .prefix("zoi-temp-git-")
            .suffix(".pkg.lua")
            .tempfile()?;
        use std::io::Write;
        temp_file.write_all(pkg_lua_content.as_bytes())?;
        let temp_path = temp_file.into_temp_path();

        let repo_name = format!("git:{}", git_source);

        return Ok(ResolvedSource {
            path: temp_path.to_path_buf(),
            source_type: SourceType::GitRepo(repo_name.clone()),
            repo_name: Some(repo_name),
            registry_handle: None,
            sharable_manifest: None,
            git_sha: None,
        });
    }

    let resolved_source = if source.starts_with("@git/") {
        let full_path_str = source.trim_start_matches("@git/");
        let parts: Vec<&str> = full_path_str.split('/').collect();

        if parts.len() < 2 {
            return Err(anyhow!(
                "Invalid git source. Use @git/<repo-name>/<path/to/pkg>"
            ));
        }

        let repo_name = parts[0];
        let nested_path_parts = &parts[1..];
        let pkg_name = nested_path_parts
            .last()
            .ok_or_else(|| anyhow!("Empty path in git source"))?;

        let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
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
            return Err(anyhow!(
                "Package '{}' not found in git repo '{}' (expected: {})",
                nested_path_str,
                repo_name,
                path.display()
            ));
        }
        println!(
            "Warning: using external git repo '{}{}' not from official Zoi database.",
            "@git/".yellow(),
            repo_name.yellow()
        );
        let git_repo_root = home_dir
            .join(".zoi")
            .join("pkgs")
            .join("git")
            .join(repo_name);
        let git_sha = get_git_head_sha(&git_repo_root);

        ResolvedSource {
            path,
            source_type: SourceType::GitRepo(repo_name.to_string()),
            repo_name: Some(format!("git/{}", repo_name)),
            registry_handle: Some("local".to_string()),
            sharable_manifest: None,
            git_sha,
        }
    } else if source.starts_with("http://") || source.starts_with("https://") {
        if crate::pkg::offline::is_offline() {
            return Err(anyhow!(
                "Cannot download package definition from URL '{}': Zoi is in offline mode.",
                source
            ));
        }
        download_from_url(download_source_for_explicit_path(source, path_part))?
    } else if let Some(path_part) = path_part {
        let path = PathBuf::from(path_part);
        if !path.exists() {
            return Err(anyhow!("Local file not found at '{path_part}'"));
        }
        ResolvedSource {
            path,
            source_type: SourceType::LocalFile,
            repo_name: None,
            registry_handle: Some("local".to_string()),
            sharable_manifest: None,
            git_sha: None,
        }
    } else {
        find_package_in_db(&request, quiet)?
    };

    let pkg_for_alt_check = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {:?}",
                resolved_source.path
            )
        })?,
        None,
        quiet,
    )?;

    if let Some(alt_source) = pkg_for_alt_check.alt {
        println!("Found 'alt' source. Resolving from: {}", alt_source.cyan());

        let mut alt_resolved_source =
            if alt_source.starts_with("http://") || alt_source.starts_with("https://") {
                println!("Downloading 'alt' source from: {}", alt_source.cyan());
                let client = crate::utils::get_http_client()?;
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
                                return Err(anyhow!(
                                    "Failed to download file after {} attempts: {}",
                                    attempt,
                                    e
                                ));
                            }
                        }
                    }
                };
                if !response.status().is_success() {
                    return Err(anyhow!(
                        "Failed to download alt source (HTTP {}): {}",
                        response.status(),
                        alt_source
                    ));
                }

                let content = response.text()?;
                let mut temp_file = tempfile::Builder::new()
                    .prefix("zoi-alt-")
                    .suffix(".pkg.lua")
                    .tempfile()?;
                use std::io::Write;
                temp_file.write_all(content.as_bytes())?;
                let temp_path = temp_file.into_temp_path();

                resolve_source_recursive(
                    temp_path.to_str().ok_or_else(|| {
                        anyhow!(
                            "Temporary path contains invalid UTF-8 characters: {:?}",
                            temp_path
                        )
                    })?,
                    depth + 1,
                    max_depth,
                    quiet,
                )?
            } else {
                resolve_source_recursive(&alt_source, depth + 1, max_depth, quiet)?
            };

        if resolved_source.source_type == SourceType::OfficialRepo {
            alt_resolved_source.source_type = SourceType::OfficialRepo;
        }

        return Ok(alt_resolved_source);
    }

    Ok(resolved_source)
}

#[cfg(test)]
mod tests {
    use super::download_source_for_explicit_path;

    #[test]
    fn test_download_source_for_explicit_http_channel_uses_base_url() {
        let source = "http://127.0.0.1:8000/test.pkg.lua@stable";
        let path_part = Some("http://127.0.0.1:8000/test.pkg.lua");
        assert_eq!(
            download_source_for_explicit_path(source, path_part),
            "http://127.0.0.1:8000/test.pkg.lua"
        );
    }

    #[test]
    fn test_download_source_for_plain_http_source_uses_original() {
        let source = "http://127.0.0.1:8000/test.pkg.lua";
        assert_eq!(
            download_source_for_explicit_path(source, None),
            "http://127.0.0.1:8000/test.pkg.lua"
        );
    }
}
