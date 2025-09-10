use crate::pkg::types;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

pub fn run(pkg_path: &Path, target_dir: Option<&str>) -> Result<(), Box<dyn Error>> {
    let pkg: types::Package =
        crate::pkg::lua_parser::parse_lua_package(pkg_path.to_str().unwrap(), None)?;

    println!("Found package: '{}'", pkg.name.bold());
    println!("Git repository: {}", pkg.git.cyan());

    let mut git_url = pkg.git.clone();
    if !git_url.ends_with(".git") {
        git_url.push_str(".git");
    }

    let final_target = target_dir.unwrap_or(&pkg.name);
    let target_path = Path::new(final_target);

    if target_path.exists() {
        if target_path.join(".git").exists() {
            println!(
                "Directory '{}' already exists and is a git repository. Pulling latest changes...",
                final_target.bold()
            );
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.green} {msg}")?,
            );
            pb.set_message(format!("Pulling changes for {}...", pkg.name));

            let mut command = Command::new("git");
            command.arg("pull");
            command.current_dir(target_path);

            let output = command.output()?;
            pb.finish_and_clear();

            if !output.status.success() {
                io::stdout().write_all(&output.stdout)?;
                io::stderr().write_all(&output.stderr)?;
                return Err("`git pull` command failed.".into());
            }
        } else {
            return Err(format!(
                "Directory '{}' already exists and is not a git repository.",
                final_target
            )
            .into());
        }
    } else {
        println!("Cloning into directory: '{}'...", final_target.bold());

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.green} {msg}")?,
        );
        pb.set_message(format!("Cloning {}...", pkg.name));

        let mut command = Command::new("git");
        command.arg("clone").arg(&git_url).arg(final_target);

        let output = command.output()?;
        pb.finish_and_clear();

        if !output.status.success() {
            io::stdout().write_all(&output.stdout)?;
            io::stderr().write_all(&output.stderr)?;
            return Err("`git clone` command failed.".into());
        }
    }

    match crate::pkg::telemetry::posthog_capture_event("clone", &pkg, env!("CARGO_PKG_VERSION")) {
        Ok(true) => println!("{} telemetry sent", "Info:".green()),
        Ok(false) => (),
        Err(e) => eprintln!("{} telemetry failed: {}", "Warning:".yellow(), e),
    }

    Ok(())
}
