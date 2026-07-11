use crate::utils;
use colored::*;

pub fn run(branch: &str, status: &str, number: &str, commit: &str, build_no: Option<&str>) {
    println!("{} Zoi version information", "::".bold().blue());
    utils::print_info("Branch", branch);
    utils::print_info("Status", status);
    utils::print_info("Number", number);
    utils::print_info("Commit", commit.green());
    if let Some(b) = build_no {
        utils::print_info("Build Number", b);
    }
}
