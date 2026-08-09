//! Logic for the `version` command.

use colored::Colorize;

use crate::utils;

/// Run the version command.
pub fn run(branch: &str, status: &str, number: &str, commit: &str,) {
    println!("{} Zoi version information", "::".bold().blue());
    utils::print_info("Branch", branch,);
    utils::print_info("Status", status,);
    utils::print_info("Number", number,);
    utils::print_info("Commit", commit.green(),);
}
