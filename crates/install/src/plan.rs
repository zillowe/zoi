use crate::resolver::InstallNode;
use crate::util;
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use zoi_core::types;

use std::path::PathBuf;

/// Details about a pre-built package available for download.
#[derive(Clone)]
pub struct PrebuiltDetails {
    /// Information about the pre-built archive.
    pub info: types::PrebuiltInfo,
    /// The size of the archive to be downloaded, in bytes.
    pub download_size: u64,
    /// The estimated size of the package once installed, in bytes.
    pub installed_size: u64,
}

/// Represents the action to be taken for installing a package.
#[derive(Clone)]
pub enum InstallAction {
    /// Download the pre-built archive and install it.
    DownloadAndInstall(PrebuiltDetails),
    /// Install from a local archive file.
    InstallFromArchive(PathBuf),
    /// Build the package from source and install it.
    BuildAndInstall,
}

/// Creates an execution plan for installing the resolved dependency graph.
///
/// This function decides the Install Action for each package:
/// - Download and Install: If a pre-built archive exists in the registry for
///   the target platform and the user didn't force a build.
/// - Build and Install: If no pre-built archive is available, or if the
///   user explicitly requested a build (via `--build` or `--type source`).
///
/// It utilizes `rayon` for parallel evaluation of pre-built availability across
/// mirrors and registries.
///
/// # Errors
///
/// Returns an error if the plan cannot be created.
pub fn create_install_plan<S: std::hash::BuildHasher + Sync>(
    graph: &HashMap<String, InstallNode, S>,
    build_type: Option<&str>,
    build: bool,
) -> Result<HashMap<String, InstallAction>> {
    let plan: HashMap<String, InstallAction> = graph
        .par_iter()
        .map(|(id, node)| {
            let is_archive = std::path::Path::new(&node.source)
                .extension()
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("zpa") || ext.eq_ignore_ascii_case("zsa")
                });

            if (build
                || (build_type.is_some()
                    && build_type != Some("pre-compiled")
                    && build_type != Some("pre-built")))
                && !is_archive
            {
                return (id.clone(), InstallAction::BuildAndInstall);
            }

            if is_archive {
                return (
                    id.clone(),
                    InstallAction::InstallFromArchive(PathBuf::from(&node.source)),
                );
            }

            let action = match util::find_prebuilt_info(node) {
                Ok(Some(info)) => {
                    let (down_size, inst_size) =
                        util::get_package_sizes(&node.pkg, &node.registry_handle, &node.version);

                    InstallAction::DownloadAndInstall(PrebuiltDetails {
                        info,
                        download_size: down_size,
                        installed_size: inst_size,
                    })
                }
                Ok(None) => InstallAction::BuildAndInstall,
                Err(e) => {
                    eprintln!(
                        "Error finding prebuilt info for {}: {}. Assuming build.",
                        node.pkg.name, e
                    );
                    InstallAction::BuildAndInstall
                }
            };
            (id.clone(), action)
        })
        .collect();

    Ok(plan)
}
