use super::structs::{FinalMetadata, PlatformAsset, ResolvedInstallation};
use crate::pkg::{lua_parser, resolve, types};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

pub fn run(package_file: &Path, install_type: Option<String>) -> Result<(), Box<dyn Error>> {
    println!("Generating metadata for: {}", package_file.display());

    let (package_template, version, _, _) =
        resolve::resolve_package_and_version(package_file.to_str().unwrap())?;

    let best_method_template = if let Some(t) = &install_type {
        package_template
            .installation
            .iter()
            .find(|m| &m.install_type == t)
            .ok_or_else(|| format!("Installation method '{}' not found", t))?
    } else {
        let method_priority = ["com_binary", "binary", "source"];
        method_priority
            .iter()
            .find_map(|t| {
                package_template
                    .installation
                    .iter()
                    .find(|m| m.install_type == *t)
            })
            .ok_or("No suitable installation method found")?
    };

    let mut installation = ResolvedInstallation {
        install_type: best_method_template.install_type.clone(),
        ..Default::default()
    };

    if best_method_template.install_type == "source" {
        installation.git = Some(best_method_template.url.clone());
        installation.tag = best_method_template.tag.clone();
        installation.branch = best_method_template.branch.clone();

        let mut build_commands_map = HashMap::new();
        let mut binary_path_map = HashMap::new();

        let platforms_to_process = if best_method_template.platforms.contains(&"all".to_string()) {
            vec![
                "linux-amd64",
                "linux-arm64",
                "windows-amd64",
                "windows-arm64",
                "macos-amd64",
                "macos-arm64",
            ]
            .into_iter()
            .map(String::from)
            .collect()
        } else {
            best_method_template.platforms.clone()
        };

        for platform_str in &platforms_to_process {
            let parsed_for_platform = lua_parser::parse_lua_package_for_platform(
                package_file.to_str().unwrap(),
                platform_str,
            )?;

            let method_for_platform = parsed_for_platform
                .installation
                .iter()
                .find(|m| m.install_type == "source")
                .ok_or("Could not find source install method in platform-specific parse")?;

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
        let platforms_to_process = if best_method_template.platforms.contains(&"all".to_string()) {
            vec![
                "linux-amd64",
                "linux-arm64",
                "windows-amd64",
                "windows-arm64",
                "macos-amd64",
                "macos-arm64",
            ]
            .into_iter()
            .map(String::from)
            .collect()
        } else {
            best_method_template.platforms.clone()
        };

        for platform_str in &platforms_to_process {
            let parsed_for_platform = lua_parser::parse_lua_package_for_platform(
                package_file.to_str().unwrap(),
                platform_str,
            )?;

            let method_for_platform = parsed_for_platform
                .installation
                .iter()
                .find(|m| m.install_type == best_method_template.install_type)
                .ok_or("Could not find chosen install method in platform-specific parse")?;

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

    let final_metadata = FinalMetadata {
        name: package_template.name,
        version,
        description: package_template.description,
        repo: package_template.repo,
        website: package_template.website,
        license: package_template.license,
        git: package_template.git,
        man_url: package_template.man,
        maintainer: package_template.maintainer,
        author: package_template.author,
        installation,
        bins: package_template.bins,
    };

    let json_output = serde_json::to_string_pretty(&final_metadata)?;
    let meta_filename = format!("{}.meta.json", final_metadata.name);
    let output_path = package_file.with_file_name(meta_filename);

    fs::write(&output_path, json_output)?;

    println!(
        "Successfully generated metadata at: {}",
        output_path.display()
    );

    Ok(())
}
