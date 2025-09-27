use crate::pkg::types;
use crate::utils;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::error::Error;
use std::io::Read;

pub fn send_telemetry(event: &str, pkg: &types::Package) {
    match crate::pkg::telemetry::posthog_capture_event(event, pkg, env!("CARGO_PKG_VERSION")) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }
}

pub fn display_updates(pkg: &types::Package, yes: bool) -> Result<bool, Box<dyn Error>> {
    if let Some(updates) = &pkg.updates {
        if updates.is_empty() {
            return Ok(true);
        }
        println!("\n{}", "Important Updates:".bold().yellow());
        for update in updates {
            let type_str = match update.update_type {
                types::UpdateType::Change => "Change".blue(),
                types::UpdateType::Vulnerability => "Vulnerability".red().bold(),
                types::UpdateType::Update => "Update".green(),
            };
            println!("  - [{}] {}", type_str, update.message);
        }

        if !utils::ask_for_confirmation("\nDo you want to continue?", yes) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn check_for_conflicts(pkg: &types::Package, yes: bool) -> Result<(), Box<dyn Error>> {
    let installed_packages = crate::pkg::local::get_installed_packages()?;

    if pkg.conflicts.is_some() || pkg.bins.is_some() {
        let mut conflict_messages = Vec::new();

        if let Some(conflicts_with) = &pkg.conflicts {
            for conflict_pkg_name in conflicts_with {
                let is_zoi_conflict = installed_packages
                    .iter()
                    .any(|p| &p.name == conflict_pkg_name);

                if is_zoi_conflict {
                    conflict_messages.push(format!(
                        "Package '{}' conflicts with installed package '{}'.",
                        pkg.name.cyan(),
                        conflict_pkg_name.cyan()
                    ));
                } else if utils::command_exists(conflict_pkg_name) {
                    conflict_messages.push(format!(
                        "Package '{}' conflicts with existing command '{}' on your system.",
                        pkg.name.cyan(),
                        conflict_pkg_name.cyan()
                    ));
                }
            }
        }

        if let Some(bins_provided) = &pkg.bins {
            for bin in bins_provided {
                for installed_pkg in &installed_packages {
                    if let Some(installed_bins) = &installed_pkg.bins
                        && installed_bins.contains(bin)
                    {
                        conflict_messages.push(format!(
                                "Binary '{}' provided by '{}' is already provided by installed package '{}'.",
                                bin.cyan(),
                                pkg.name.cyan(),
                                installed_pkg.name.cyan()
                            ));
                    }
                }
            }
        }

        let unique_messages: HashSet<String> = conflict_messages.into_iter().collect();
        if !unique_messages.is_empty() {
            println!("{}", "Conflict Detected:".red().bold());
            for msg in unique_messages {
                println!("- {}", msg);
            }
            if !utils::ask_for_confirmation(
                "Do you want to continue with the installation anyway?",
                yes,
            ) {
                return Err("Operation aborted by user due to conflicts.".into());
            }
        }
        return Ok(());
    }

    Ok(())
}

pub fn get_filename_from_url(url: &str) -> &str {
    url.split('/').next_back().unwrap_or("")
}

pub fn find_method<'a>(
    pkg: &'a types::Package,
    type_name: &str,
    platform: &str,
) -> Option<&'a types::InstallationMethod> {
    pkg.installation.iter().find(|m| {
        m.install_type == type_name && crate::utils::is_platform_compatible(platform, &m.platforms)
    })
}

pub fn download_file_with_progress(url: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if url.starts_with("http://") {
        println!(
            "{} downloading over insecure HTTP: {}",
            "Warning:".yellow(),
            url
        );
    }
    println!("Downloading from: {url}");

    let client = crate::utils::build_blocking_http_client(60)?;
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
                        "Failed to download '{}' after {} attempts: {}",
                        url, attempt, e
                    )
                    .into());
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(format!("Failed to download (HTTP {}): {}", response.status(), url).into());
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")? 
        .progress_chars("#>- T"));

    let mut downloaded_bytes = Vec::new();
    let mut stream = response;
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        downloaded_bytes.extend_from_slice(&buffer[..bytes_read]);
        pb.inc(bytes_read as u64);
    }
    pb.finish_with_message("Download complete.");
    Ok(downloaded_bytes)
}
