use super::build;
use anyhow::Result;

pub fn run(args: build::BuildCommand) -> Result<()> {
    crate::pkg::package::test::run(&args)
}
