use crate::pkg::install::{InstallMode, run_installation};
use crate::pkg::{local, resolve, service, types};
use crate::utils;
use colored::*;
use std::collections::HashSet;
use std::error::Error;

pub fn run(package_name: &str, yes: bool) -> Result<(), Box<dyn Error>> {
    let (pkg, _, _) = resolve::resolve_package_and_version(package_name)?;

    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", package_name).into());
    }

    let is_installed = local::is_package_installed(package_name, pkg.scope)?.is_some();

    if !is_installed {
        println!("Service '{}' is not installed.", package_name.cyan());
        if utils::ask_for_confirmation("Do you want to install it now?", yes) {
            println!("Installing '{}'...", package_name.cyan());
            let mut processed_deps = HashSet::new();
            run_installation(
                package_name,
                InstallMode::PreferBinary,
                false,
                types::InstallReason::Direct,
                yes,
                false,
                &mut processed_deps,
            )?;
        } else {
            return Err("Service not installed, aborting.".into());
        }
    }

    service::start_service(&pkg)
}
