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

    let posthog_host = option_env!("POSTHOG_API_HOST");
    let zoi_registry = option_env!("ZOI_DEFAULT_REGISTRY");
    let about_packager_author = option_env!("ZOI_ABOUT_PACKAGER_AUTHOR");
    let about_packager_email = option_env!("ZOI_ABOUT_PACKAGER_EMAIL");
    let about_packager_homepage = option_env!("ZOI_ABOUT_PACKAGER_HOMEPAGE");

    let has_build_info =
        posthog_host.is_some_and(|s| !s.is_empty()) || zoi_registry.is_some_and(|s| !s.is_empty());

    if has_build_info {
        println!("\n  {}", "Build Information".green());
        if let Some(host) = posthog_host
            && !host.is_empty()
        {
            println!("  {:<19}{}", "Telemetry Host:".cyan(), host);
        }
        if let Some(registry) = zoi_registry
            && !registry.is_empty()
        {
            println!("  {:<19}{}", "Default Registry:".cyan(), registry);
        }
    }

    let has_packager_info = about_packager_author.is_some_and(|s| !s.is_empty())
        || about_packager_email.is_some_and(|s| !s.is_empty())
        || about_packager_homepage.is_some_and(|s| !s.is_empty());

    if has_packager_info {
        println!("\n  {}", "Packager Information".green());
        if let Some(author) = about_packager_author
            && !author.is_empty()
        {
            println!("  {:<19}{}", "Packager:".cyan(), author);
        }
        if let Some(email) = about_packager_email
            && !email.is_empty()
        {
            println!("  {:<19}{}", "Email:".cyan(), email);
        }
        if let Some(homepage) = about_packager_homepage
            && !homepage.is_empty()
        {
            println!("  {:<19}{}", "Homepage:".cyan(), homepage);
        }
    }

    println!();
}
