use crate::pkg::types;
use crate::utils;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

pub fn install_manual_if_available(
    pkg: &types::Package,
    version: &str,
    registry_handle: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(url) = &pkg.man {
        println!("Downloading manual from {}...", url);
        let content = reqwest::blocking::get(url)?.bytes()?;

        let version_dir = crate::pkg::local::get_package_version_dir(
            pkg.scope,
            registry_handle,
            &pkg.repo,
            &pkg.name,
            version,
        )?;
        fs::create_dir_all(&version_dir)?;

        let extension = if url.ends_with(".md") { "md" } else { "txt" };
        let man_path = version_dir.join(format!("man.{}", extension));

        fs::write(man_path, &content)?;
        println!("Manual for '{}' installed.", pkg.name);
    }
    Ok(())
}

pub fn run_post_install_hooks(pkg: &types::Package) -> Result<(), Box<dyn Error>> {
    if let Some(hooks) = &pkg.post_install {
        println!("\n{}", "Running post-installation commands...".bold());
        let platform = utils::get_platform()?;

        for hook in hooks {
            if utils::is_platform_compatible(&platform, &hook.platforms) {
                for cmd_str in &hook.commands {
                    println!("Executing: {}", cmd_str.cyan());

                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                            .template("{spinner:.green} {msg}")?,
                    );
                    pb.set_message(format!("Running: {}", cmd_str));

                    let output = if cfg!(target_os = "windows") {
                        Command::new("pwsh").arg("-Command").arg(cmd_str).output()?
                    } else {
                        Command::new("bash").arg("-c").arg(cmd_str).output()?
                    };

                    pb.finish_and_clear();

                    if !output.status.success() {
                        io::stdout().write_all(&output.stdout)?;
                        io::stderr().write_all(&output.stderr)?;
                        return Err(format!("Post-install command failed: '{}'", cmd_str).into());
                    } else {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if !stdout.trim().is_empty() {
                            println!("{}", stdout.trim());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
