use crate::pkg::pin;
use colored::*;

pub fn run(package: &str, version: &str) {
    if let Err(e) = run_pin_logic(package, version) {
        eprintln!("{}: {}", "Pin failed".red().bold(), e);
    }
}

fn run_pin_logic(name: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut pinned_packages = pin::get_pinned_packages()?;

    if pinned_packages.iter().any(|p| p.name == name) {
        println!("Package '{name}' is already pinned. Unpin it first to change the version.");
        return Ok(());
    }

    let new_pin = pin::PinnedPackage {
        name: name.to_string(),
        version: version.to_string(),
    };
    pinned_packages.push(new_pin);
    pin::write_pinned_packages(&pinned_packages)?;

    println!("Pinned {}@{}", name.green(), version.yellow());
    Ok(())
}
