use crate::pkg::{resolve, types};
use crate::utils;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

fn get_cache_root() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    Ok(home_dir.join(".zoi").join("cache"))
}

fn ensure_binary_is_cached(pkg: &types::Package) -> Result<PathBuf, Box<dyn Error>> {
    let cache_dir = get_cache_root()?;
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

    println!(
        "No cached binary found for '{}'. Downloading...",
        pkg.name.cyan()
    );
    fs::create_dir_all(&cache_dir)?;

    let platform = utils::get_platform()?;
    let method = pkg
        .installation
        .iter()
        .find(|m| {
            m.install_type == "binary" && utils::is_platform_compatible(&platform, &m.platforms)
        })
        .ok_or("No compatible 'binary' installation method found for this package.")?;

    let mut url = method.url.replace("{version}", &pkg.version);
    url = url.replace("{name}", &pkg.name);
    url = url.replace("{platform}", &platform);

    println!("Downloading from: {url}");

    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(format!("Failed to download binary: HTTP {}", response.status()).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

    let mut dest = File::create(&bin_path)?;
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(&buffer[..bytes_read])?;
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message("Download complete.");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(bin_path)
}

pub fn run(source: &str, args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let resolved_source = resolve::resolve_source(source)?;

    utils::print_repo_warning(&resolved_source.repo_name);

    let content = fs::read_to_string(&resolved_source.path)?;
    let pkg: types::Package = serde_yaml::from_str(&content)?;

    let bin_path = ensure_binary_is_cached(&pkg)?;

    println!("\n--- Executing '{}' ---\n", pkg.name.bold());
    let status = Command::new(bin_path).args(args).status()?;

    if let Some(code) = status.code() {
        std::process::exit(code);
    }

    Ok(())
}
