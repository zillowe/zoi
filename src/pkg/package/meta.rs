use super::structs::{FinalMetadata, PlatformAsset, ResolvedInstallation};
use crate::pkg::{lua_parser, resolve, types};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

pub fn run(
    package_file: &Path,
    install_type: Option<String>,
    version_override: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    println!("Generating metadata for: {}", package_file.display());

    let temp_pkg = lua_parser::parse_lua_package(package_file.to_str().unwrap(), None)?;
    let default_version = resolve::get_default_version(&temp_pkg)?;

    let version = version_override
        .map(|s| s.to_string())
        .unwrap_or(default_version);

    let package_template =
        lua_parser::parse_lua_package(package_file.to_str().unwrap(), Some(&version))?;

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
            )?;

            let method_for_platform = parsed_for_platform
                .installation
                .iter()
                .find(|m| m.install_type == "source")
                .ok_or("Could not find source install method in platform-specific parse")?;

            let os_part = platform_str.split('-').next().unwrap_or(platform_str);

            if let Some(cmds) = &method_for_platform.build_commands
                && !build_commands_map.contains_key(os_part)
            {
                build_commands_map.insert(os_part.to_string(), cmds.clone());
            }
            if let Some(path) = &method_for_platform.binary_path
                && !binary_path_map.contains_key(os_part)
            {
                binary_path_map.insert(os_part.to_string(), path.clone());
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

    installation.files = best_method_template.files.as_ref().map(|groups| {
        groups
            .iter()
            .map(|g| super::structs::FileGroup {
                platforms: g.platforms.clone(),
                files: g
                    .files
                    .iter()
                    .map(|f| super::structs::FileCopy {
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
