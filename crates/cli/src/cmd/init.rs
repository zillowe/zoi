//! Implementation of the `init` command for creating a new `zoi.lua`.

use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Result, anyhow};
use colored::Colorize;
use dialoguer::{Input, Select};

/// Derives a Lua-safe project name from a directory name.
///
/// Project names end up in a Lua string literal, so anything that could break
/// out of one has to be neutralised rather than written verbatim.
fn sanitize_project_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();

    // A name must not start with a digit, because `project({ name = "1.0" })`
    // is fine but the value is often reused in shell and Lua contexts where a
    // leading dash or dot reads as a flag or a relative path.
    let trimmed = cleaned.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() {
        "my-project".to_string()
    } else {
        trimmed
    }
}

/// Returns the name of the current directory, if it can be determined.
fn current_dir_name() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
        .filter(|n| !n.is_empty())
}

/// Builds the contents of a new `zoi.lua`.
///
/// The block is deliberately minimal: `packages({})` is present so that
/// `zoi install --save` has a block to append to without having to create one.
fn template(project_name: &str) -> String {
    format!(
        "project({{\n    name = \"{project_name}\"\n}})\n\npackages({{\n    \
         -- Record project dependencies here, or add them with:\n    --   zoi \
         install --save <package>\n}})\n"
    )
}

/// Runs the `init` command, writing a new `zoi.lua` in the current directory.
///
/// Refuses to overwrite an existing `zoi.lua`. Every decision is confirmed
/// first when a terminal is attached, since this writes a file the user may not
/// have wanted in that particular directory.
///
/// # Errors
///
/// Returns an error if a `zoi.lua` already exists, if the project name cannot
/// be determined or is declined at the prompt, or if the file cannot be
/// written.
pub fn run(yes: bool, force: bool) -> Result<()> {
    let config_path = Path::new("zoi.lua");

    if config_path.exists() && !force {
        return Err(anyhow!(
            "A 'zoi.lua' already exists in this directory. Run 'zoi init \
             --force' to overwrite it."
        ));
    }

    // Prefer an explicit name, fall back to the directory, and only prompt when
    // neither is usable.
    let default_name = current_dir_name().map_or_else(
        || "my-project".to_string(),
        |n| sanitize_project_name(&n)
    );

    let project_name = if yes {
        default_name
    } else if std::io::stdin().is_terminal() {
        let input = Input::<String>::new()
            .with_prompt("Project name")
            .default(default_name)
            .allow_empty(false)
            .interact_text()?;
        sanitize_project_name(input.trim())
    } else {
        // Non-interactive without --yes: take the directory name rather than
        // blocking on a prompt nobody can answer.
        default_name
    };

    let contents = template(&project_name);

    if config_path.exists() {
        println!(
            "{} Overwriting existing 'zoi.lua'...",
            "Warning:".yellow().bold()
        );
    }

    // Show what is about to be written before writing it. A file the user has
    // never seen is hard to review after the fact.
    if !yes && std::io::stdout().is_terminal() {
        println!("\n{}", contents.dimmed());
        let choice = Select::new()
            .with_prompt("Write this zoi.lua?")
            .items(["Yes", "No, cancel"])
            .default(0)
            .interact()?;
        if choice == 1 {
            println!("{}", "Aborted.".yellow());
            return Ok(());
        }
    }

    fs::write(config_path, &contents)?;

    println!(
        "{} Created {} for project '{}'",
        "Success:".green().bold(),
        "zoi.lua".cyan(),
        project_name
    );
    println!(
        "\nAdd a dependency with:\n    {}",
        "zoi install --save <package>".dimmed()
    );

    Ok(())
}
