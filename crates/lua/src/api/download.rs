use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zoi_core::utils;

pub fn download_with_progress(url: &str, dest_path: &Path, quiet: bool) -> Result<(), mlua::Error> {
    if url.starts_with("http://") && !quiet {
        println!(
            "{}: downloading over insecure HTTP: {}",
            "Warning:".yellow(),
            url
        );
    }

    let client = utils::get_http_client().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    let mut attempt = 0u32;
    let mut response = loop {
        attempt += 1;
        match client.get(url).send() {
            Ok(resp) => {
                if !resp.status().is_success() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Failed to download {}: {}",
                        url,
                        resp.status()
                    )));
                }
                break resp;
            }
            Err(e) => {
                if attempt < 3 {
                    if !quiet {
                        eprintln!("Download failed ({}). Retrying...", e);
                    }
                    zoi_core::utils::retry_backoff_sleep(attempt);
                    continue;
                } else {
                    return Err(mlua::Error::RuntimeError(e.to_string()));
                }
            }
        }
    };

    let total_size = response.content_length().unwrap_or(0);

    let pb = if !quiet {
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} {msg:30.cyan} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {elapsed_precise})")
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?
            .progress_chars("=>-"));

        let filename = url.split('/').next_back().unwrap_or("file");
        pb.set_message(format!("Downloading {}", filename));
        Some(pb)
    } else {
        None
    };

    let mut dest_file =
        fs::File::create(dest_path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    let mut buffer = [0; 8192];
    let mut downloaded = 0;

    while let Ok(n) = response.read(&mut buffer) {
        if n == 0 {
            break;
        }
        dest_file
            .write_all(&buffer[..n])
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        downloaded += n as u64;
        if let Some(ref p) = pb {
            p.set_position(downloaded);
        }
    }

    if let Some(p) = pb {
        p.finish_and_clear();
    }

    Ok(())
}
