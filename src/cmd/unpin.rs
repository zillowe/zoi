use crate::pkg::pin;
use colored::*;

pub fn run(source: &str) {
    if let Err(e) = run_unpin_logic(source) {
        eprintln!("{}: {}", "Unpin failed".red().bold(), e);
    }
}

fn run_unpin_logic(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut pinned_packages = pin::get_pinned_packages()?;

    let initial_len = pinned_packages.len();
    pinned_packages.retain(|p| p.source != source);

    if pinned_packages.len() == initial_len {
        println!("Package '{source}' was not pinned.");
        return Ok(());
    }

    pin::write_pinned_packages(&pinned_packages)?;

    println!("Unpinned {}", source.green());
    Ok(())
}
