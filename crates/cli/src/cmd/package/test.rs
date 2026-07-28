use super::build;
use anyhow::Result;

pub fn run(args: build::BuildCommand) -> Result<()> {
    if args.install_deps {
        build::install_dependencies_for_build(&args, true)?;
    }
    crate::pkg::package::test::run(&args)
}
