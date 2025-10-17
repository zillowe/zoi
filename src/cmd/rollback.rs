use crate::pkg::{self, resolve};
use anyhow::Result;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    let (pkg, _, _, _, _) = resolve::resolve_package_and_version(package_name)?;
    pkg::rollback::run(&pkg.name, yes)
}
