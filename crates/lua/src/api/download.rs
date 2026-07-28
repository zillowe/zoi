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

pub fn add_download_util(lua: &mlua::Lua, quiet: bool) -> Result<(), mlua::Error> {
    let download_fn = lua.create_function(
        move |lua, (url, out_name, hash): (String, Option<String>, Option<String>)| {
            let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
            let build_dir = Path::new(&build_dir_str);

            let filename = out_name.unwrap_or_else(|| {
                url.split('/')
                    .next_back()
                    .unwrap_or("download.tmp")
                    .to_string()
            });

            let dest_path = build_dir.join(&filename);

            download_with_progress(&url, &dest_path, quiet)?;

            if let Some(hash_spec) = hash {
                let parts: Vec<&str> = hash_spec.splitn(2, '-').collect();
                let (algo, expected_hash) = if parts.len() == 2 {
                    (parts[0], parts[1])
                } else {
                    ("sha512", hash_spec.as_str())
                };

                let hash_algo = match zoi_core::hash::HashAlgorithm::from_name(algo) {
                    Some(a) => a,
                    None => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Unsupported hash algorithm: {}",
                            algo
                        )));
                    }
                };

                let actual_hash = zoi_core::hash::calculate_file_hash(&dest_path, hash_algo)
                    .map_err(|e| {
                        mlua::Error::RuntimeError(format!("Failed to calculate hash: {}", e))
                    })?;

                if !actual_hash.eq_ignore_ascii_case(expected_hash) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Hash mismatch for {}. Expected: {}-{}, Got: {}-{}",
                        filename, algo, expected_hash, algo, actual_hash
                    )));
                }
            }

            Ok(filename)
        },
    )?;

    let utils_table: mlua::Table = lua.globals().get("UTILS")?;
    utils_table.set("DOWNLOAD", download_fn)?;

    Ok(())
}
