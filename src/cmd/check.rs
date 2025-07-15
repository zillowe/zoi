use crate::utils;
use colored::*;
use std::io::{stdout, Write};
pub fn run() {
    println!(
        "\n{}",
        "--- Checking for Essential Tools ---".yellow().bold()
    );

    let essential_tools = ["git"];
    let mut all_tools_found = true;

    for tool in essential_tools {
        print!("Checking for {}... ", tool.cyan());
        let _ = stdout().flush();

        if utils::command_exists(tool) {
            println!("{}", "OK".green());
        } else {
            println!("{}", "MISSING".red());
            all_tools_found = false;
        }
    }

    println!();

    if all_tools_found {
        println!("{}", "All essential tools checked are installed.".green());
    } else {
        println!(
            "{}",
            "One or more essential tools are missing. Please install them.".red()
        );
    }
    println!();
}
