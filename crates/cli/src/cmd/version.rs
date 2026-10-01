//! Logic for the `version` command.

use colored::Colorize;

use crate::utils;

/// Run the version command.
pub fn run(branch: &str, status: &str, number: &str, commit: &str) {
    println!("{} Zoi version information", "::".bold().blue());
    // - Print the ZFVM identifier, not the long-form name. `branch` is one of
    // - Prod/Dev/Spec/Pub, and reporting "Development" here contradicted
    // - `zoi about`, which renders the same fields through
    // - `format_version_summary`. Both now agree.
    utils::print_info("Branch", branch);
    utils::print_info("Status", status);
    utils::print_info("Number", number);
    utils::print_info("Commit", commit.green());
}
