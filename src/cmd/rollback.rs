use crate::pkg;
use anyhow::Result;

pub fn run(package_name: &str, yes: bool) -> Result<()> {
    pkg::rollback::run(package_name, yes)
}
