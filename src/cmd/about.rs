use crate::utils;
use colored::*;

const DESCRIPTION: &str = "Zoi - Universal Package Manager & Environment Setup Tool.\n  Part of the Zillowe Development Suite (ZDS)";
const AUTHOR: &str = "Zusty < Zillowe Foundation";
const HOMEPAGE: &str = "https://zillowe.qzz.io/zds/zoi";
const DOCS: &str = "https://zillowe.qzz.io/docs/zds/zoi";
const GITREPO: &str = "https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi";
const EMAIL: &str = "contact@zillowe.qzz.io";
const LICENSE: &str = "Apache 2.0";

pub fn run(branch: &str, status: &str, number: &str, commit: &str) {
    let full_version_string = utils::format_version_full(branch, status, number, commit);

    println!("\n  {}\n", DESCRIPTION.green());

    println!("  {:<12}{}", "Version:".cyan(), full_version_string);
    println!("  {:<12}{}", "Author:".cyan(), AUTHOR);
    println!("  {:<12}{}", "Homepage:".cyan(), HOMEPAGE);
    println!("  {:<12}{}", "Docs:".cyan(), DOCS);
    println!("  {:<12}{}", "Email:".cyan(), EMAIL);
    println!("  {:<12}{}", "GitLab:".cyan(), GITREPO);
    println!("  {:<12}{}", "License:".cyan(), LICENSE);
    println!();
}
