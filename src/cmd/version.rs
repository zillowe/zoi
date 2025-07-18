use crate::utils;
use colored::*;

pub fn run(branch: &str, status: &str, number: &str, commit: &str) {
    println!("{}", "--- Zoi Version ---".yellow().bold());
    utils::print_info("Branch", branch);
    utils::print_info("Status", status);
    utils::print_info("Number", number);
    utils::print_info("Commit", commit.green());
}
