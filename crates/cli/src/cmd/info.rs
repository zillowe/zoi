//! Command for displaying system and configuration information.

use anyhow::Result;
use colored::Colorize;

use crate::{pkg, utils};

/// Runs the 'info' command.
///
/// Displays detailed information about the system, platform,
/// package managers, and Zoi configuration.
///
/// # Errors
///
/// Returns an error if the platform information or configuration cannot be
/// retrieved.
pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    commit: &str,
) -> Result<(),> {
    let branch_short = if branch == "Production" {
        "Prod."
    } else if branch == "Development" {
        "Dev."
    } else if branch == "Public" {
        "Pub."
    } else if branch == "Special" {
        "Spec."
    } else {
        branch
    };

    println!("{} System information", "::".bold().blue());

    let platform = crate::pkg::utils::get_platform()?;
    let parts: Vec<&str,> = platform.split('-',).collect();
    let os = parts.first().copied().unwrap_or("unknown",);
    let arch = parts.get(1,).copied().unwrap_or("unknown",);

    utils::print_aligned_info("OS", os,);
    utils::print_aligned_info("Architecture", arch,);

    if os == "linux"
        && let Some(dist,) = crate::pkg::utils::get_linux_distribution()
    {
        utils::print_aligned_info("Distribution", &dist,);
    }

    let config = pkg::config::read_config()?;
    let native_pm = config.native_package_manager;
    let all_pms = config.package_managers.unwrap_or_default();

    if all_pms.is_empty() {
        utils::print_aligned_info(
            "Package Managers",
            "Not available (run 'zoi sync')",
        );
    } else {
        let pm_list: Vec<String,> = all_pms
            .into_iter()
            .map(|pm| {
                if Some(pm.clone(),) == native_pm {
                    format!("{} (native)", pm.green())
                } else {
                    pm
                }
            },)
            .collect();
        let pm_list_str = pm_list.join(", ",);
        utils::print_aligned_info("Package Managers", &pm_list_str,);
    }

    let tel = if config.telemetry_enabled {
        "Enabled".green()
    } else {
        "Disabled".yellow()
    };
    utils::print_aligned_info("Telemetry", &tel.to_string(),);

    let key_with_colon = format!("{}:", "Version");
    println!(
        "{:<18}{} {} {} {}",
        key_with_colon.cyan(),
        branch_short,
        status,
        number,
        commit.green()
    );
    Ok((),)
}
