use crate::pkg::{local, resolve, service, types};
use std::error::Error;

pub fn run(package_name: &str) -> Result<(), Box<dyn Error>> {
    if local::is_package_installed(package_name, types::Scope::User)?.is_none()
        && local::is_package_installed(package_name, types::Scope::System)?.is_none()
    {
        return Err(format!("Service '{}' is not installed.", package_name).into());
    }

    let (pkg, _, _) = resolve::resolve_package_and_version(package_name)?;

    if pkg.package_type != types::PackageType::Service {
        return Err(format!("Package '{}' is not a service.", package_name).into());
    }

    service::stop_service(&pkg)
}
