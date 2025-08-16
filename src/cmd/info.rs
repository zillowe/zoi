use crate::pkg;
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

    println!("{}", "--- System Information ---".yellow().bold());

    let platform = utils::get_platform().unwrap_or_else(|e| e.to_string());
    let parts: Vec<&str> = platform.split('-').collect();
    let os = parts.first().cloned().unwrap_or("unknown");
    let arch = parts.get(1).cloned().unwrap_or("unknown");

    utils::print_aligned_info("OS", os);
    utils::print_aligned_info("Architecture", arch);

    if os == "linux" {
        if let Some(dist) = utils::get_linux_distribution() {
            utils::print_aligned_info("Distribution", &dist);
        }
    }

    let config = pkg::config::read_config();
    let (native_pm, all_pms) = if let Ok(config) = config {
        (
            config.native_package_manager,
            config.package_managers.unwrap_or_default(),
        )
    } else {
        (None, Vec::new())
    };

    if !all_pms.is_empty() {
        let pm_list: Vec<String> = all_pms
            .into_iter()
            .map(|pm| {
                if Some(pm.clone()) == native_pm {
                    format!("{} (native)", pm.green())
                } else {
                    pm
                }
            })
            .collect();
        let pm_list_str = pm_list.join(", ");
        utils::print_aligned_info("Package Managers", &pm_list_str);
    } else {
        utils::print_aligned_info("Package Managers", "Not available (run 'zoi sync')");
    }

    if let Ok(config) = pkg::config::read_config() {
        let tel = if config.telemetry_enabled {
            "Enabled".green()
        } else {
            "Disabled".yellow()
        };
        utils::print_aligned_info("Telemetry", &tel.to_string());
    }

    let key_with_colon = format!("{}:", "Version");
    println!(
        "{:<18}{} {} {} {}",
        key_with_colon.cyan(),
        _branch_short,
        status,
        number,
        commit.green()
    );
}
