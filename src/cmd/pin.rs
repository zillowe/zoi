use crate::pkg::pin;
use colored::*;

pub fn run(package: &str) {
    if let Err(e) = run_pin_logic(package) {
        eprintln!("{}: {}", "Pin failed".red().bold(), e);
    }
}

fn run_pin_logic(package: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = package.splitn(2, '@');
    let name = parts.next().unwrap_or("").to_string();
    let version = parts.next().unwrap_or("").to_string();

    if name.is_empty() || version.is_empty() {
        return Err("Invalid package format. Use 'name@version'.".into());
    }

    let mut pinned_packages = pin::get_pinned_packages()?;

    if pinned_packages.iter().any(|p| p.name == name) {
        println!("Package '{name}' is already pinned. Unpin it first to change the version.");
        return Ok(());
    }

    let new_pin = pin::PinnedPackage { name: name.clone(), version: version.clone() };
    pinned_packages.push(new_pin);
    pin::write_pinned_packages(&pinned_packages)?;

    println!("Pinned {}@{}", name.green(), version.yellow());
    Ok(())
}
