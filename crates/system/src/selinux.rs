use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub fn restore_context<P: AsRef<Path>>(path: P) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }

    // Check if restorecon exists
    if !zoi_core::utils::command_exists("restorecon") {
        return Ok(());
    }

    println!(
        "Restoring SELinux context for {}...",
        path.as_ref().display()
    );

    let status = Command::new("restorecon")
        .arg("-R") // Recursive
        .arg("-v") // Verbose
        .arg(path.as_ref())
        .status()?;

    if !status.success() {
        eprintln!("Warning: restorecon failed for {}", path.as_ref().display());
    }

    Ok(())
}
