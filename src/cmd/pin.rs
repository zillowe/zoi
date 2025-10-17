use crate::pkg::{pin, resolve};
use anyhow::Result;
use colored::*;

pub fn run(source: &str, version: &str) {
    if let Err(e) = run_pin_logic(source, version) {
        eprintln!("{}: {}", "Pin failed".red().bold(), e);
    }
}

fn run_pin_logic(source: &str, version: &str) -> Result<()> {
    let (pkg, _, _, _, _) = resolve::resolve_package_and_version(source)?;
    let mut pinned_packages = pin::get_pinned_packages()?;

    if pinned_packages.iter().any(|p| p.source == pkg.name) {
        println!(
            "Package '{}' is already pinned. Unpin it first to change the version.",
            pkg.name
        );
        return Ok(());
    }

    let new_pin = pin::PinnedPackage {
        source: pkg.name.clone(),
        version: version.to_string(),
    };
    pinned_packages.push(new_pin);
    pin::write_pinned_packages(&pinned_packages)?;

    println!("Pinned {}@{}", pkg.name.green(), version.yellow());
    Ok(())
}
