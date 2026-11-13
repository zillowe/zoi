use std::fs;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;
use zoi_core::sysroot;

use crate::generation::GenerationManager;

/// Where `zoi system distro build` writes its temporary os-release marker,
/// relative to the target root.
///
/// It is not `/etc/os-release`: the distribution's own packages (Parlex's
/// `filesystem`) install that, and a stray file there would collide with
/// them. [`zoi_core::utils::get_linux_distribution_info`] reads this file
/// only when the target has no real os-release yet.
pub const BOOTSTRAP_OS_RELEASE: &str = "etc/zoi/bootstrap-os-release";

/// Marks `target` as a `ZoiOS` root for the duration of the build and makes
/// it the sysroot.
///
/// Installing `zoios = true` packages requires the target to identify as
/// `ZoiOS`. Until a package provides the real `/etc/os-release`, that is done
/// with a temporary marker at [`BOOTSTRAP_OS_RELEASE`]; see
/// [`finalize_zoios_marker`]. The hostname goes to `/etc/hostname`.
///
/// # Errors
///
/// Returns an error if the marker or hostname cannot be written.
pub fn initialize_zoios_marker(
    target: &Path,
    hostname: Option<&str>,
    dry_run: bool
) -> Result<()> {
    if dry_run {
        println!(
            "  {} Would mark {} as ZoiOS and set its hostname",
            "[DRY-RUN]".dimmed(),
            target.display()
        );
        return Ok(());
    }

    sysroot::set_sysroot(target.to_path_buf());
    fs::create_dir_all(target.join("etc/zoi"))?;

    let marker = "ID=zoios\n\
                  NAME=ZoiOS\n\
                  PRETTY_NAME=\"ZoiOS\"\n";
    fs::write(target.join(BOOTSTRAP_OS_RELEASE), marker)?;

    let hostname = hostname.unwrap_or("zoios");
    fs::write(target.join("etc/hostname"), format!("{hostname}\n"))?;
    Ok(())
}

/// Resolves the temporary marker once the packages are installed.
///
/// If a package installed `/etc/os-release` (or `/usr/lib/os-release`), the
/// marker is removed. Otherwise it becomes `/etc/os-release`, so the target
/// is still recognized as `ZoiOS`.
///
/// # Errors
///
/// Returns an error if the marker cannot be moved or removed.
pub fn finalize_zoios_marker(target: &Path, dry_run: bool) -> Result<()> {
    let marker = target.join(BOOTSTRAP_OS_RELEASE);
    if dry_run || !marker.exists() {
        return Ok(());
    }
    let etc = target.join("etc/os-release");
    let has_os_release = etc.exists()
        || etc.is_symlink()
        || target.join("usr/lib/os-release").exists();
    if has_os_release {
        fs::remove_file(&marker)?;
    } else {
        fs::rename(&marker, &etc)?;
    }
    Ok(())
}

/// Records the first system generation once a target has been provisioned.
///
/// # Errors
///
/// Returns an error if the first generation record cannot be written.
pub fn finalize_first_generation(
    target: &Path,
    packages: Vec<String>,
    dry_run: bool
) -> Result<()> {
    if dry_run {
        println!(
            "  {} Would create first generation in {}/var/lib/zoi/generations",
            "[DRY-RUN]".dimmed(),
            target.display()
        );
        return Ok(());
    }

    let gen_manager =
        GenerationManager::with_root(target.join("var/lib/zoi/generations"))?;
    let id = gen_manager.create_generation(packages)?;
    gen_manager.activate_generation(id)?;
    Ok(())
}
