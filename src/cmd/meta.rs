use crate::pkg::package::structs::{
    FileCopy, FileGroup, FinalMetadata, PlatformAsset, ResolvedInstallation,
};
use crate::pkg::{lua_parser, resolve, types};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn generate_metadata(
    package_file: &Path,
    version_override: Option<&str>,
    repo: String,
) -> Result<FinalMetadata> {
    println!("Generating metadata for: {}", package_file.display());

    let temp_pkg = lua_parser::parse_lua_package(package_file.to_str().unwrap(), None)
        .map_err(|e| anyhow!(e.to_string()))?;
    let default_version =
        resolve::get_default_version(&temp_pkg).map_err(|e| anyhow!(e.to_string()))?;

    let version = version_override
        .map(|s| s.to_string())
        .unwrap_or(default_version);

    let package_template =
        lua_parser::parse_lua_package(package_file.to_str().unwrap(), Some(&version))
            .map_err(|e| anyhow!(e.to_string()))?;

    let method_priority = ["com_binary", "binary", "source", "installer", "script"];
    let best_method_template = method_priority
        .iter()
        .find_map(|t| {
            package_template
                .installation
                .iter()
                .find(|m| m.install_type == *t)
        })
        .ok_or_else(|| anyhow!("No suitable installation method found"))?;

    let platforms_to_process = {
        let mut platforms = Vec::new();
        if best_method_template.platforms.contains(&"all".to_string()) {
            platforms.extend(vec![
                "linux-amd64".to_string(),
                "linux-arm64".to_string(),
                "windows-amd64".to_string(),
                "windows-arm64".to_string(),
                "macos-amd64".to_string(),
                "macos-arm64".to_string(),
            ]);
        } else {
            for p in &best_method_template.platforms {
                if p.contains('-') {
                    platforms.push(p.clone());
                } else {
                    platforms.push(format!("{}-amd64", p));
                    platforms.push(format!("{}-arm64", p));
                }
            }
        }
        platforms
    };

    let mut installation = ResolvedInstallation {
        install_type: best_method_template.install_type.clone(),
        ..Default::default()
    };

    if best_method_template.install_type == "source" {
        installation.git = Some(best_method_template.url.clone());
        installation.tag = best_method_template.tag.clone();
        installation.branch = best_method_template.branch.clone();
        installation.docker_image = best_method_template.docker_image.clone();

        let mut build_commands_map = HashMap::new();
        let mut binary_path_map = HashMap::new();

        for platform_str in &platforms_to_process {
            let parsed_for_platform = lua_parser::parse_lua_package_for_platform(
                package_file.to_str().unwrap(),
                platform_str,
                Some(&version),
            )
            .map_err(|e| anyhow!(e.to_string()))?;

            let method_for_platform = parsed_for_platform
                .installation
                .iter()
                .find(|m| m.install_type == "source")
                .ok_or_else(|| {
                    anyhow!("Could not find source install method in platform-specific parse")
                })?;

            if let Some(cmds) = &method_for_platform.build_commands {
                build_commands_map.insert(platform_str.clone(), cmds.clone());
            }
            if let Some(path) = &method_for_platform.binary_path {
                binary_path_map.insert(platform_str.clone(), path.clone());
            }
        }
        installation.build_commands = Some(build_commands_map);
        installation.binary_path = Some(binary_path_map);
    } else {
        let checksum_map: HashMap<String, String> =
            if let Some(checksums) = &best_method_template.checksums {
                match checksums {
                    types::Checksums::Url(url) => {
                        println!("Fetching checksums from {}", url);
                        let resp = reqwest::blocking::get(url)?.text()?;
                        let mut map = HashMap::new();
                        for line in resp.lines() {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() == 2 {
                                map.insert(parts[1].to_string(), parts[0].to_string());
                            }
                        }
                        map
                    }
                    types::Checksums::List { items, .. } => items
                        .iter()
                        .map(|item| (item.file.clone(), item.checksum.clone()))
                        .collect(),
                }
            } else {
                HashMap::new()
            };

        let mut assets = Vec::new();

        for platform_str in &platforms_to_process {
            let parsed_for_platform = lua_parser::parse_lua_package_for_platform(
                package_file.to_str().unwrap(),
                platform_str,
                Some(&version),
            )
            .map_err(|e| anyhow!(e.to_string()))?;

            let method_for_platform = parsed_for_platform
                .installation
                .iter()
                .find(|m| m.install_type == best_method_template.install_type)
                .ok_or_else(|| {
                    anyhow!("Could not find chosen install method in platform-specific parse")
                })?;

            let url = method_for_platform.url.clone();
            let filename = url.split('/').next_back().unwrap_or("").to_string();
            let checksum = checksum_map.get(&filename).cloned();

            let signature_url = if let Some(sigs) = &method_for_platform.sigs {
                sigs.iter()
                    .find(|s| s.file == filename)
                    .map(|s| s.sig.clone())
            } else {
                None
            };

            assets.push(PlatformAsset {
                platform: platform_str.to_string(),
                url,
                checksum,
                signature_url,
            });
        }
        installation.assets = assets;
    }

    installation.files = best_method_template.files.as_ref().map(|groups| {
        groups
            .iter()
            .map(|g| FileGroup {
                platforms: g.platforms.clone(),
                files: g
                    .files
                    .iter()
                    .map(|f| FileCopy {
                        source: f.source.clone(),
                        destination: f.destination.clone(),
                    })
                    .collect(),
            })
            .collect()
    });

    let final_metadata = FinalMetadata {
        name: package_template.name,
        version,
        description: package_template.description,
        repo,
        website: package_template.website,
        license: package_template.license,
        git: package_template.git,
        man_url: package_template.man,
        maintainer: package_template.maintainer,
        author: package_template.author,
        installation,
        bins: package_template.bins,
    };

    Ok(final_metadata)
}

pub fn run(package_name: &str, output: Option<&str>, version: Option<&str>) -> Result<()> {
    let resolved_source =
        resolve::resolve_source(package_name).map_err(|e| anyhow!(e.to_string()))?;
    let repo_name = resolved_source.repo_name.clone().unwrap_or_default();
    let final_metadata = generate_metadata(&resolved_source.path, version, repo_name)?;

    let json_output = serde_json::to_string_pretty(&final_metadata)?;

    if let Some(output_path) = output {
        fs::write(output_path, json_output)?;
        println!("Successfully generated metadata at: {}", output_path);
    } else {
        println!("{}", json_output);
    }

    Ok(())
}
