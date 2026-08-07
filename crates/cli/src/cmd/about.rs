//! The about command displays information about Zoi.
use crate::utils;
use colored::*;

/// The description of the application.
const DESCRIPTION: &str = "Zoi - Advanced Package Manager & Environment Orchestrator.\n  Part of the Zillowe Development Suite (ZDS)";
/// The author of the application.
const AUTHOR: &str = "Zusty < Zillowe Foundation";
/// The homepage of the application.
const HOMEPAGE: &str = "https://zillowe.qzz.io/zds/zoi";
/// The documentation URL for the application.
const DOCS: &str = "https://zillowe.qzz.io/docs/zds/zoi";
/// The git repository URL for the application.
const GITREPO: &str = "https://gitlab.com/zillowe/zillwen/zusty/zoi";
/// The contact email for the application.
const EMAIL: &str = "contact@zillowe.qzz.io";
/// The license of the application.
const LICENSE: &str = "Apache 2.0";

/// Executes the about command.
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

    println!("\n  By continuing using Zoi, you agree to our Privacy Policy and Terms of Service.");
    println!("  Privacy Policy and Terms of Service can be found on our website.");

    println!();
}
