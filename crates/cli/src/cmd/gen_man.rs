//! Implementation of the `zoi gen-man` command.
//!
//! This command generates man pages for the Zoi CLI and all its subcommands.

use std::path::{Path, PathBuf};
use std::{env, fs, io};

use clap::{Command, CommandFactory};
use clap_mangen::Man;

use crate::cli::Cli;

/// Runs the 'gen-man' command.
///
/// Generates man pages for the Zoi CLI and all its subcommands in the 'manuals'
/// directory.
///
/// # Errors
///
/// Returns an error if the 'manuals' directory cannot be created or if there is
/// an issue generating the man pages.
pub fn run() -> io::Result<()> {
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| "manuals".to_string());
    let out_path = PathBuf::from(out_dir);
    fs::create_dir_all(&out_path)?;

    let app = Cli::command();
    println!("Generating man pages in {}...", out_path.display());

    generate_man_page(&app, &out_path)?;

    for sub_command in app.get_subcommands() {
        generate_man_pages_recursive(sub_command, &out_path, app.get_name())?;
    }

    println!(
        "\nSuccessfully generated man pages in '{}'.",
        out_path.display()
    );
    Ok(())
}

/// Recursively generates man pages for a command and all its subcommands.
fn generate_man_pages_recursive(
    cmd: &Command,
    out_path: &Path,
    parent_name: &str
) -> io::Result<()> {
    if cmd.is_hide_set() {
        return Ok(());
    }

    let full_name = format!("{}-{}", parent_name, cmd.get_name());
    let leaked_name: &'static str = Box::leak(full_name.into_boxed_str());
    let new_cmd = cmd.clone().name(leaked_name);
    generate_man_page(&new_cmd, out_path)?;

    for sub_cmd in new_cmd.get_subcommands() {
        generate_man_pages_recursive(sub_cmd, out_path, leaked_name)?;
    }

    Ok(())
}

/// Generates a single man page for a command.
fn generate_man_page(app: &Command, out_path: &Path) -> io::Result<()> {
    let name = app.get_name();
    let out_file = out_path.join(format!("{name}.1"));

    let man = Man::new(app.clone());
    let mut buffer = Vec::<u8>::new();
    man.render(&mut buffer)?;

    fs::write(&out_file, &buffer)?;
    println!("- {}", out_file.display());

    Ok(())
}
