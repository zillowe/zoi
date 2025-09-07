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
    pub sharable_manifest: Option<types::SharableInstallManifest>,
}

#[derive(Debug, Default)]
struct PackageRequest {
    repo: Option<String>,
    name: String,
    version_spec: Option<String>,
}

pub(crate) fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
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
            repo: None,
            name,
            version_spec: None,
        });
    }

    let mut repo = None;
    let name: &str;
    let mut version_spec = None;

    let mut main_part = source_str;

    if let Some(at_pos) = source_str.rfind('@')
        && at_pos > 0
    {
        let (pkg_part, ver_part) = source_str.split_at(at_pos);
        main_part = pkg_part;
        version_spec = Some(ver_part[1..].to_string());
    }

    if main_part.starts_with('@') {
        let s = main_part.trim_start_matches('@');
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
        name = main_part;
    }

    if name.is_empty() {
        return Err("Invalid source string: package name is empty.".into());
    }

    Ok(PackageRequest {
        repo,
        name: name.to_lowercase(),
        version_spec,
    })
}

fn find_package_in_db(request: &PackageRequest) -> Result<ResolvedSource, Box<dyn Error>> {
    let db_root = get_db_root()?;

    let search_repos = if let Some(r) = &request.repo {
        vec![r.clone()]
    } else {
        config::read_config()?.repos
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

        for repo_name in &search_repos {
            let path = db_root
                .join(repo_name)
                .join(&request.name)
                .join(format!("{}.pkg.lua", pkg_name));

            if path.exists() {
                let pkg: types::Package =
                    crate::pkg::lua_parser::parse_lua_package(path.to_str().unwrap())?;
                let major_repo = repo_name.split('/').next().unwrap_or("").to_lowercase();
                let source_type = if major_repo == "core"
                    || major_repo == "main"
                    || major_repo == "extra"
                    || major_repo == "zillowe"
                {
                    SourceType::OfficialRepo
                } else {
                    SourceType::UntrustedRepo(repo_name.clone())
                };
                found_packages.push(FoundPackage {
                    path,
                    source_type,
                    repo_name: repo_name.clone(),
                    description: pkg.description,
                });
            }
        }
    } else {
        for repo_name in &search_repos {
            let repo_path = db_root.join(repo_name);
            if !repo_path.is_dir() {
                continue;
            }
            for entry in WalkDir::new(&repo_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_dir() && e.file_name() == request.name.as_str())
            {
                let pkg_dir_path = entry.path();
                let pkg_file_path = pkg_dir_path.join(format!("{}.pkg.lua", request.name));

                if pkg_file_path.exists() {
                    let pkg: types::Package =
                        crate::pkg::lua_parser::parse_lua_package(pkg_file_path.to_str().unwrap())?;
                    let major_repo = repo_name.split('/').next().unwrap_or("").to_lowercase();
                    let source_type = if major_repo == "core"
                        || major_repo == "main"
                        || major_repo == "extra"
                        || major_repo == "zillowe"
                    {
                        SourceType::OfficialRepo
                    } else {
                        SourceType::UntrustedRepo(repo_name.clone())
                    };

                    let full_repo_path = pkg_dir_path
                        .strip_prefix(&db_root)
                        .unwrap_or(pkg_dir_path)
                        .to_string_lossy()
                        .to_string();

                    found_packages.push(FoundPackage {
                        path: pkg_file_path,
                        source_type,
                        repo_name: full_repo_path,
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
        println!(
            "Found package '{}' in repo '{}'",
            request.name, chosen.repo_name
        );
        Ok(ResolvedSource {
            path: chosen.path.clone(),
            source_type: chosen.source_type.clone(),
            repo_name: Some(chosen.repo_name.clone()),
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
        sharable_manifest: None,
    })
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

pub fn get_default_version(pkg: &types::Package) -> Result<String, Box<dyn Error>> {
    if let Some(pinned_version) = pin::get_pinned_version(&pkg.name)? {
        println!(
            "Using pinned version '{}' for {}.",
            pinned_version.yellow(),
            pkg.name.cyan()
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

    get_default_version(pkg)
}

pub fn resolve_source(source: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    resolve_source_recursive(source, 0)
}

pub fn resolve_package_and_version(
    source_str: &str,
) -> Result<
    (
        types::Package,
        String,
        Option<types::SharableInstallManifest>,
        PathBuf,
    ),
    Box<dyn Error>,
> {
    let request = parse_source_string(source_str)?;
    let resolved_source = resolve_source_recursive(source_str, 0)?;
    let pkg_lua_path = resolved_source.path.clone();

    let mut pkg =
        crate::pkg::lua_parser::parse_lua_package(resolved_source.path.to_str().unwrap())?;
    if let Some(repo_name) = resolved_source.repo_name.clone() {
        pkg.repo = repo_name;
    }

    let version_string = get_version_for_install(&pkg, &request.version_spec)?;

    pkg.version = Some(version_string.clone());
    Ok((
        pkg,
        version_string,
        resolved_source.sharable_manifest,
        pkg_lua_path,
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
            "@{}/{}@{}",
            sharable_manifest.repo, sharable_manifest.name, sharable_manifest.version
        );
        let mut resolved_source = resolve_source_recursive(&new_source, depth + 1)?;
        resolved_source.sharable_manifest = Some(sharable_manifest);
        return Ok(resolved_source);
    }

    let request = parse_source_string(source)?;

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
            sharable_manifest: None,
        }
    } else {
        find_package_in_db(&request)?
    };

    let pkg_for_alt_check =
        crate::pkg::lua_parser::parse_lua_package(resolved_source.path.to_str().unwrap())?;

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
