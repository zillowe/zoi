use crate::pkg;
use std::error::Error;

pub fn run(package_name: &str, yes: bool) -> Result<(), Box<dyn Error>> {
    pkg::rollback::run(package_name, yes)
}
