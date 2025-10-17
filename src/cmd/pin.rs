use crate::pkg::pin;
use anyhow::Result;
use colored::*;

pub fn run(source: &str, version: &str) {
    if let Err(e) = run_pin_logic(source, version) {
        eprintln!("{}: {}", "Pin failed".red().bold(), e);
    }
}

fn run_pin_logic(source: &str, version: &str) -> Result<()> {
    let mut pinned_packages = pin::get_pinned_packages()?;

    if pinned_packages.iter().any(|p| p.source == source) {
        println!("Package '{source}' is already pinned. Unpin it first to change the version.");
        return Ok(());
    }

    let new_pin = pin::PinnedPackage {
        source: source.to_string(),
        version: version.to_string(),
    };
    pinned_packages.push(new_pin);
    pin::write_pinned_packages(&pinned_packages)?;

    println!("Pinned {}@{}", source.green(), version.yellow());
    Ok(())
}
