use crate::pkg::{local, resolve};
use std::error::Error;

pub fn run(libs: bool, cflags: bool, packages: &[String]) -> Result<(), Box<dyn Error>> {
    if packages.is_empty() {
        return Err("No packages specified.".into());
    }

    for package_name in packages {
        let (pkg, _, _) = resolve::resolve_package_and_version(package_name)?;
        if local::is_package_installed(&pkg.name, pkg.scope)?.is_none() {
            return Err(format!("Package '{}' is not installed.", pkg.name).into());
        }

        if let Some(pkg_config) = pkg.pkg_config {
            if libs {
                print!("{} ", pkg_config.libs);
            }
            if cflags {
                print!("{} ", pkg_config.cflags);
            }
        } else {
            return Err(format!(
                "Package '{}' does not have pkg-config information.",
                pkg.name
            )
            .into());
        }
    }
    println!();
    Ok(())
}
