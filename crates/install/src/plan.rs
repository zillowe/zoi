use crate::resolver::InstallNode;
use crate::util;
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use zoi_core::types;

use std::path::PathBuf;

#[derive(Clone)]
pub struct PrebuiltDetails {
    pub info: types::PrebuiltInfo,
    pub download_size: u64,
    pub installed_size: u64,
}

#[derive(Clone)]
pub enum InstallAction {
    DownloadAndInstall(PrebuiltDetails),
    InstallFromArchive(PathBuf),
    BuildAndInstall,
}

pub fn create_install_plan(
    graph: &HashMap<String, InstallNode>,
    build_type: Option<&str>,
    build: bool,
) -> Result<HashMap<String, InstallAction>> {
    let plan: HashMap<String, InstallAction> = graph
        .par_iter()
        .map(|(id, node)| {
            if build
                || (build_type.is_some()
                    && build_type != Some("pre-compiled")
                    && build_type != Some("pre-built"))
            {
                return (id.clone(), InstallAction::BuildAndInstall);
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
