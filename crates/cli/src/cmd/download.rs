//! Implementation of the `download` command for downloading package archives.

use anyhow::{Result, anyhow};
use colored::Colorize;
use std::path::PathBuf;
use zoi_core::cache;
use zoi_install::resolver::resolve_dependency_graph;
use zoi_install::util;

/// Type of download to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadType {
    /// Download a pre-built archive (.zpa).
    Archive,
    /// Download a source bundle (.zsa).
    Source,
}

/// Runs the `download` command to download a package archive or source bundle.
///
/// # Errors
///
/// Returns an error if:
/// - The package cannot be resolved.
/// - The requested download type (Archive or Source) is not available for the package.
/// - The download fails or file system operations fail.
pub fn run(
    package_source: &str,
    download_type: DownloadType,
    output_dir: Option<PathBuf>,
) -> Result<()> {
    println!(
        "{} Resolving package '{}' for download...",
        "::".bold().blue(),
        package_source.cyan()
    );

    let (graph, _) = resolve_dependency_graph(
        &[package_source.to_string()],
        None,
        false,
        true,
        false,
        None,
        true,
        None,
    )?;

    if graph.nodes.is_empty() {
        return Err(anyhow!("Could not resolve package '{package_source}'"));
    }

    // Find the direct package node
    let node = graph
        .nodes
        .values()
        .find(|n| matches!(n.reason, zoi_core::types::InstallReason::Direct))
        .ok_or_else(|| anyhow!("Could not find target package in resolution graph"))?;

    println!(
        "{} Resolved to {} v{}",
        "::".bold().green(),
        node.pkg.name.cyan(),
        node.version.yellow()
    );

    let info = if download_type == DownloadType::Source {
        util::find_source_bundle_info(node)?.ok_or_else(|| {
            anyhow!("No source bundle (.zsa) information found for this package in the registry.")
        })?
    } else {
        util::find_prebuilt_info(node)?.ok_or_else(|| {
            anyhow!(
                "No pre-built archive (.zpa) information found for this package in the registry."
            )
        })?
    };

    let filename = info
        .final_url
        .split('/')
        .next_back()
        .unwrap_or("package.archive");

    let dest_path = if let Some(dir) = output_dir {
        std::fs::create_dir_all(&dir)?;
        dir.join(filename)
    } else {
        let cache_root = cache::get_archive_cache_root()?;
        std::fs::create_dir_all(&cache_root)?;
        cache_root.join(filename)
    };

    println!(
        "{} Downloading to: {}",
        "::".bold().blue(),
        dest_path.display()
    );

    let (down_size, _) = util::get_package_sizes(&node.pkg, &node.registry_handle, &node.version);

    util::download_file_with_progress(&info.final_url, &dest_path, None, Some(down_size))?;

    println!(
        "{} Successfully downloaded: {}",
        "::".bold().green(),
        dest_path.display()
    );

    Ok(())
}
