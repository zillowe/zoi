//! Implementation of the `clone` command for cloning a package's git
//! repository.

use std::process::Command;

use anyhow::{Result, anyhow};
use colored::Colorize;

use crate::pkg::{resolve, types};

/// Runs the `clone` command to clone a package's git repository.
///
/// # Errors
///
/// Returns an error if the package cannot be resolved, if its metadata cannot
/// be parsed, if it has no git repository defined, or if the `git clone`
/// command fails.
///
/// # Panics
///
/// This function does not explicitly panic.
pub fn run(
    package_name: &str,
    location: Option<String,>,
    yes: bool,
) -> Result<(),> {
    println!(
        "{} Resolving package '{}' for cloning...",
        "::".bold().blue(),
        package_name.cyan().bold()
    );

    let resolved_source =
        resolve::resolve_source(package_name, None, false, yes,)?;

    let pkg: types::Package = crate::pkg::lua::parser::parse_lua_package(
        resolved_source.path.to_str().ok_or_else(|| {
            let p = resolved_source.path.display();
            anyhow!("Path contains invalid UTF-8 characters: {p}")
        },)?,
        None,
        None,
        false,
    )?;

    if pkg.git.is_empty() {
        return Err(anyhow!(
            "Package '{}' does not have a git repository defined in its \
             metadata.",
            pkg.name
        ),);
    }

    let git_url = &pkg.git;
    let target_location = location.unwrap_or(pkg.name,);

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

    let mut git_cmd = Command::new("git",);
    git_cmd.arg("clone",).arg(git_url,).arg(&target_location,);

    let status = git_cmd
        .status()
        .map_err(|e| anyhow!("Failed to execute git clone: {e}"),)?;

    if status.success() {
        println!("\n{}", "Successfully cloned repository.".green());
    } else {
        let code = status
            .code()
            .map_or_else(|| "unknown".to_string(), |c| c.to_string(),);
        return Err(anyhow!("git clone failed with exit code {code}"),);
    }

    Ok((),)
}
