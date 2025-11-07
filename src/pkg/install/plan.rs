use crate::pkg::install::resolver::InstallNode;
use crate::pkg::install::util;
use crate::pkg::types;
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;

pub struct PrebuiltDetails {
    pub info: types::PrebuiltInfo,
    pub download_size: u64,
}

pub enum InstallAction {
    DownloadAndInstall(PrebuiltDetails),
    BuildAndInstall,
}

pub fn create_install_plan(
    graph: &HashMap<String, InstallNode>,
) -> Result<HashMap<String, InstallAction>> {
    let plan: HashMap<String, InstallAction> = graph
        .par_iter()
        .map(|(id, node)| {
            let action = match util::find_prebuilt_info(node) {
                Ok(Some(info)) => {
                    let size = if let Some(size_url) = &info.size_url {
                        util::get_expected_size(size_url).unwrap_or_else(|e| {
                            eprintln!(
                                "Warning: could not fetch size for {}: {}. Falling back to metadata.",
                                node.pkg.name,
                                e
                            );
                            node.pkg.archive_size.unwrap_or(0)
                        })
                    } else {
                        node.pkg.archive_size.unwrap_or(0)
                    };

                    InstallAction::DownloadAndInstall(PrebuiltDetails {
                        info,
                        download_size: size,
                    })
                }
                Ok(None) => InstallAction::BuildAndInstall,
                Err(e) => {
                    eprintln!("Error finding prebuilt info for {}: {}. Assuming build.", node.pkg.name, e);
                    InstallAction::BuildAndInstall
                }
            };
            (id.clone(), action)
        })
        .collect();

    Ok(plan)
}
