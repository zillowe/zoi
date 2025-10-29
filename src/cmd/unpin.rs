use crate::pkg::{pin, resolve};
use anyhow::Result;
use colored::*;

pub fn run(source: &str) {
    if let Err(e) = run_unpin_logic(source) {
        eprintln!("{}: {}", "Unpin failed".red().bold(), e);
    }
}

fn run_unpin_logic(source: &str) -> Result<()> {
    let (pkg, _, _, _, _) = resolve::resolve_package_and_version(source, false)?;
    let mut pinned_packages = pin::get_pinned_packages()?;

    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.source != pkg.name);

    if pinned_packages.len() == initial_len {
        println!("Package '{}' was not pinned.", pkg.name);
        return Ok(());
    }

    pin::write_pinned_packages(&pinned_packages)?;

    println!("Unpinned {}", pkg.name.green());
    Ok(())
}
