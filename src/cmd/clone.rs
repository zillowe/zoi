use crate::pkg::{resolve, types};
use anyhow::{Result, anyhow};
use colored::*;
use std::process::Command;

pub fn run(package_name: &str, location: Option<String>, yes: bool) -> Result<()> {
    println!(
        "{} Resolving package '{}' for cloning...",
        "::".bold().blue(),
        package_name.cyan().bold()
    );

    let resolved_source = resolve::resolve_source(package_name, false, yes)?;

    let pkg: types::Package = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().ok_or_else(|| {
            anyhow!(
                "Path contains invalid UTF-8 characters: {:?}",
                resolved_source.path
            )
        })?,
        None,
        false,
    )?;

    if pkg.git.is_empty() {
        return Err(anyhow!(
            "Package '{}' does not have a git repository defined in its metadata.",
            pkg.name
        ));
    }

    let git_url = &pkg.git;
    let target_location = location.unwrap_or_else(|| pkg.name.clone());

    println!(
        "{} Cloning {} into {}...",
        "::".bold().blue(),
        git_url.cyan(),
        if target_location == "." {
            "current directory".bold()
        } else {
            target_location.bold()
        }
    );

    let mut git_cmd = Command::new("git");
    git_cmd.arg("clone").arg(git_url).arg(&target_location);

    let status = git_cmd
        .status()
        .map_err(|e| anyhow!("Failed to execute git clone: {}", e))?;

    if status.success() {
        println!("\n{}", "Successfully cloned repository.".green());
    } else {
        return Err(anyhow!(
            "git clone failed with exit code {:?}",
            status.code()
        ));
    }

    Ok(())
}
