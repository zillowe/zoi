use crate::utils;
use colored::*;

pub fn run(branch: &str, status: &str, number: &str, commit: &str) {
    let _branch_short = if branch == "Production" {
        "Prod."
    } else if branch == "Development" {
        "Dev."
    } else {
        branch
    };

    println!("{}", "--- Zoi Version ---".yellow().bold());
    utils::print_info("Branch", _branch_short);
    utils::print_info("Status", status);
    utils::print_info("Number", number);
    utils::print_info("Commit", commit.green());
}
