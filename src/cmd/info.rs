use crate::utils;
use colored::*;
use std::env;

pub fn run() {
    println!("{}", "--- System Information ---".yellow().bold());

    let os = env::consts::OS;
    let arch = match env::consts::ARCH {
        other => other,
    };

    utils::print_aligned_info("OS", os);
    utils::print_aligned_info("Architecture", arch);

    if os == "linux" {
        if let Some(dist) = utils::get_linux_distribution() {
            utils::print_aligned_info("Distribution", &dist);
        }
    }
    if let Some(pm) = utils::get_native_package_manager() {
        utils::print_aligned_info("Package Manager", &pm);
    } else {
        utils::print_aligned_info("Package Manager", "Not available");
    }
}
