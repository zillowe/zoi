use crate::utils;
use anyhow::{Result, anyhow};
use colored::*;
use std::io::{Write, stdout};

pub fn run() -> Result<()> {
    println!("{}", "--- Checking for Essential Tools ---".yellow().bold());

    let essential_tools = ["git"];
    let mut all_essential_found = true;

    for tool in essential_tools {
        print!("Checking for {}... ", tool.cyan());
        let _ = stdout().flush();

        if utils::command_exists(tool) {
            println!("{}", "OK".green());
        } else {
            println!("{}", "MISSING".red());
            all_essential_found = false;
        }
    }

    if !all_essential_found {
        return Err(anyhow!(
            "One or more essential tools are missing. Please install them."
        ));
    }

    println!(
        "
{}",
        "--- Checking for Recommended Tools ---".yellow().bold()
    );
    let recommended_tools = ["less", "gpg"];
    for tool in recommended_tools {
        print!("Checking for {}... ", tool.cyan());
        let _ = stdout().flush();

        if utils::command_exists(tool) {
            println!("{}", "OK".green());
        } else {
            println!("{}", "Not Found".yellow());
        }
    }

    println!();

    println!("{}", "All essential tools are installed.".green());
    Ok(())
}
