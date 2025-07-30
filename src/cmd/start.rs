use crate::pkg::install::{run_installation, InstallMode};
use crate::pkg::{local, resolve, service, types};
use crate::utils;
use colored::*;
use std::error::Error;

pub fn run(package_name: &str, yes: bool) -> Result<(), Box<dyn Error>> {
    // Resolve the package definition first to check its type.
    let (pkg, _) = resolve::resolve_package_and_version(package_name)?;

    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", package_name).into());
    }

    let is_installed = local::is_package_installed(package_name, pkg.scope)?.is_some();

    if !is_installed {
        println!("Service '{}' is not installed.", package_name.cyan());
        if utils::ask_for_confirmation("Do you want to install it now?", yes) {
            println!("Installing '{}'...", package_name.cyan());
            run_installation(
                package_name,
                InstallMode::PreferBinary,
                false, // force
                types::InstallReason::Direct,
                yes,
            )?;
        } else {
            return Err("Service not installed, aborting.".into());
        }
    }

    // After ensuring it's installed, start the service.
    // The `pkg` object from the initial resolution is used here.
    service::start_service(&pkg)
}
