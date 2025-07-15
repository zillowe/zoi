use crate::pkg::types;
use colored::*;
use std::error::Error;
use std::path::Path;
use std::process::Command;

pub fn run(pkg_path: &Path, target_dir: Option<&str>) -> Result<(), Box<dyn Error>> {
    let content = std::fs::read_to_string(pkg_path)?;
    let pkg: types::Package = serde_yaml::from_str(&content)?;

    println!("Found package: '{}'", pkg.name.bold());
    println!("Git repository: {}", pkg.git.cyan());

    let mut git_url = pkg.git.clone();
    if !git_url.ends_with(".git") {
        git_url.push_str(".git");
    }

    let final_target = target_dir.unwrap_or(&pkg.name);

    println!("Cloning into directory: '{}'...", final_target.bold());

    let mut command = Command::new("git");
    command.arg("clone").arg(&git_url).arg(final_target);

    let status = command.status()?;

    if !status.success() {
        return Err("`git clone` command failed.".into());
    }

    Ok(())
}
