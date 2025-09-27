use crate::pkg::types;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub fn handle_source_install(
    method: &types::InstallationMethod,
    pkg: &types::Package,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "Building from source...".bold());
    let store_path = home::home_dir()
        .ok_or("No home dir")?
        .join(".zoi/pkgs/store")
        .join(&pkg.name);
    let git_path = store_path.join("git");
    let bin_path = store_path.join("bin");
    fs::create_dir_all(&bin_path)?;

    let repo_url = &method.url;
    println!("Cloning from {}...", repo_url);

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")?,
    );
    pb.set_message(format!("Cloning {}...", pkg.name));

    let output = std::process::Command::new("git")
        .arg("clone")
        .arg(repo_url)
        .arg(&git_path)
        .output()?;
    pb.finish_and_clear();

    if !output.status.success() {
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
        return Err("Failed to clone source repository.".into());
    }

    if method.tag.is_some() && method.branch.is_some() {
        return Err(
            "Invalid source method: both 'tag' and 'branch' specified. Use only one.".into(),
        );
    }
    if let Some(tag) = &method.tag {
        println!("Checking out tag {}...", tag.cyan());
        let out = std::process::Command::new("git")
            .current_dir(&git_path)
            .arg("checkout")
            .arg(format!("tags/{}", tag))
            .output()?;
        if !out.status.success() {
            io::stdout().write_all(&out.stdout)?;
            io::stderr().write_all(&out.stderr)?;
            return Err(format!("Failed to checkout tag '{}'", tag).into());
        }
    } else if let Some(branch) = &method.branch {
        println!("Checking out branch {}...", branch.cyan());
        let out = std::process::Command::new("git")
            .current_dir(&git_path)
            .arg("checkout")
            .arg(branch)
            .output()?;
        if !out.status.success() {
            io::stdout().write_all(&out.stdout)?;
            io::stderr().write_all(&out.stderr)?;
            return Err(format!("Failed to checkout branch '{}'", branch).into());
        }
    }

    if let Some(commands) = &method.build_commands {
        for cmd_str in commands {
            let final_cmd = cmd_str.replace("{prefix}", store_path.to_str().unwrap());
            println!("Executing: {}", final_cmd.cyan());

            let pb_cmd = ProgressBar::new_spinner();
            pb_cmd.set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.green} {msg}")?,
            );
            pb_cmd.set_message(format!("Running: {}", final_cmd));

            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(&final_cmd)
                .current_dir(&git_path)
                .output()?;
            pb_cmd.finish_and_clear();

            if !output.status.success() {
                io::stdout().write_all(&output.stdout)?;
                io::stderr().write_all(&output.stderr)?;
                return Err(format!("Build command failed: '{}'", final_cmd).into());
            }
        }
    }

    let entries: Vec<PathBuf> = fs::read_dir(&bin_path)
        .map_err(|e| format!("Failed to read store directory at {:?}: {}", bin_path, e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    if entries.is_empty() {
        println!(
            "{}",
            "Build completed, no binaries found in the store directory to link.".yellow()
        );
        println!("{}", "Source build and installation completed.".green());
        return Ok(());
    }

    println!("Build completed, searching for binaries in store directory to link...");

    let mut binaries_to_link: Vec<(String, PathBuf)> = Vec::new();

    if let Some(bin_names) = &pkg.bins {
        for bin_name in bin_names {
            let mut found_bin = false;
            for entry in &entries {
                let file_name = entry.file_name().unwrap().to_string_lossy();
                if file_name == *bin_name
                    || (cfg!(target_os = "windows") && file_name == format!("{}.exe", bin_name))
                {
                    binaries_to_link.push((bin_name.clone(), entry.clone()));
                    found_bin = true;
                    break;
                }
            }
            if !found_bin {
                return Err(format!(
                    "Could not find expected binary '{}' in store directory after build.",
                    bin_name
                )
                .into());
            }
        }
    } else if entries.len() == 1 {
        let bin_path = entries[0].clone();
        let bin_name = bin_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        binaries_to_link.push((bin_name, bin_path));
    } else {
        let os_specific_name = if cfg!(target_os = "windows") {
            format!("{}.exe", pkg.name)
        } else {
            pkg.name.clone()
        };

        for entry in &entries {
            if entry.file_name().unwrap().to_string_lossy() == os_specific_name {
                binaries_to_link.push((pkg.name.clone(), entry.clone()));
                break;
            }
        }
        if binaries_to_link.is_empty() && cfg!(target_os = "windows") {
            for entry in &entries {
                if entry.file_name().unwrap().to_string_lossy() == pkg.name {
                    binaries_to_link.push((pkg.name.clone(), entry.clone()));
                    break;
                }
            }
        }
    }

    if binaries_to_link.is_empty() {
        return Err(format!(
            "Build produced files in the store directory, but could not determine which binary to link for package '{}'. Specify the binary name in the 'bins' field of the package manifest.",
            pkg.name
        ).into());
    }

    for (bin_name, binary_path_in_store) in binaries_to_link {
        println!("Found built binary: {}", binary_path_in_store.display());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary_path_in_store, fs::Permissions::from_mode(0o755))?;

            let symlink_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&symlink_dir)?;
            let symlink_path = symlink_dir.join(bin_name);

            if symlink_path.exists() {
                fs::remove_file(&symlink_path)?;
            }
            std::os::unix::fs::symlink(&binary_path_in_store, &symlink_path)?;
        }

        #[cfg(windows)]
        {
            let bin_dir = home::home_dir().ok_or("No home dir")?.join(".zoi/pkgs/bin");
            fs::create_dir_all(&bin_dir)?;
            let dest_path = bin_dir.join(binary_path_in_store.file_name().unwrap());
            if dest_path.exists() {
                fs::remove_file(&dest_path)?;
            }
            fs::copy(&binary_path_in_store, &dest_path)?;
        }
    }

    println!("{}", "Source build and installation completed.".green());
    Ok(())
}
