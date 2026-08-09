//! Running tests for packages.

use anyhow::Result;

use super::build;

/// Runs tests for a package.
///
/// This command builds the package and runs its defined tests.
/// If `install_deps` is set, it will also install necessary build dependencies.
///
/// # Errors
///
/// Returns an error if the tests fail or dependencies cannot be installed.
pub fn run(args: &build::BuildCommand,) -> Result<(),> {
    if args.install_deps {
        build::install_dependencies_for_build(args, true,)?;
    }
    crate::pkg::package::test::run(args,)
}
