use crate::pkg::config;
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum SourceType {
    OfficialRepo,
    UntrustedRepo(String),
    LocalFile,
    Url,
}

#[derive(Debug)]
pub struct ResolvedSource {
    pub path: PathBuf,
    pub source_type: SourceType,
    pub repo_name: Option<String>,
}

fn get_db_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("pkgs").join("db"))
}

fn parse_db_pkg_string(pkg_str: &str) -> (Option<&str>, &str) {
    if let Some(stripped) = pkg_str.strip_prefix('@') {
        if let Some(split_index) = stripped.find('/') {
            let (repo, pkg_name) = stripped.split_at(split_index);
            return (Some(repo), &pkg_name[1..]);
        }
    }
    (None, pkg_str)
}

fn find_package_in_db(pkg_str: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    let pkg_str = pkg_str.trim();
    let (repo, pkg_name) = parse_db_pkg_string(pkg_str);
    let db_root = get_db_root()?;

    let search_repos = if let Some(r) = repo {
        vec![r.to_string()]
    } else {
        config::read_config()?.repos
    };

    for repo_name in &search_repos {
        let pkg_file_name = format!("{pkg_name}.pkg.yaml");
        let path = db_root.join(repo_name).join(&pkg_file_name);
        if path.exists() {
            println!("Found package '{pkg_name}' in repo '{repo_name}'");
            let source_type = if repo_name == "main" || repo_name == "extra" {
                SourceType::OfficialRepo
            } else {
                SourceType::UntrustedRepo(repo_name.clone())
            };
            return Ok(ResolvedSource {
                path,
                source_type,
                repo_name: Some(repo_name.clone()),
            });
        }
    }

    if repo.is_some() {
        Err(format!("Package '{pkg_name}' not found in repository '@{}'.", repo.unwrap()).into())
    } else {
        Err(format!("Package '{pkg_name}' not found in any active repositories.").into())
    }
}

fn download_from_url(url: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    println!("Downloading package definition from URL...");
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
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
        "zoi-temp-{}.yaml",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    fs::write(&temp_path, content)?;

    Ok(ResolvedSource {
        path: temp_path,
        source_type: SourceType::Url,
        repo_name: None,
    })
}

pub fn resolve_source(source: &str) -> Result<ResolvedSource, Box<dyn Error>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        download_from_url(source)
    } else if source.ends_with(".pkg.yaml") {
        let path = PathBuf::from(source);
        if !path.exists() {
            return Err(format!("Local file not found at '{source}'").into());
        }
        println!("Using local package file: {}", path.display());
        Ok(ResolvedSource {
            path,
            source_type: SourceType::LocalFile,
            repo_name: None,
        })
    } else {
        find_package_in_db(source)
    }
}
