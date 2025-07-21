use crate::pkg::pin;
use colored::*;

pub fn run(package_name: &str) {
    if let Err(e) = run_unpin_logic(package_name) {
        eprintln!("{}: {}", "Unpin failed".red().bold(), e);
    }
}

fn run_unpin_logic(package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut pinned_packages = pin::get_pinned_packages()?;

    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.name != package_name);

    if pinned_packages.len() == initial_len {
        println!("Package '{}' was not pinned.", package_name);
        return Ok(());
    }

    pin::write_pinned_packages(&pinned_packages)?;

    println!("Unpinned {}", package_name.green());
    Ok(())
}
