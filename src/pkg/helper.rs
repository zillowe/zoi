use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;
use std::io::Read;

pub enum HashType {
    Sha512,
    Sha256,
}

fn update_digest_from_reader<R: Read, D: Digest>(reader: &mut R, hasher: &mut D) -> Result<()> {
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(())
}

pub fn get_hash(source: &str, hash_type: HashType) -> Result<String> {
    let mut hasher_sha512 = Sha512::new();
    let mut hasher_sha256 = Sha256::new();

    if source.starts_with("http://") || source.starts_with("https://") {
        let client = crate::utils::get_http_client()?;
        let mut response = client.get(source).send()?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to download file from URL: {}",
                response.status()
            ));
        }
        match hash_type {
            HashType::Sha512 => {
                update_digest_from_reader(&mut response, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                update_digest_from_reader(&mut response, &mut hasher_sha256)?;
            }
        }
    } else {
        let mut file = File::open(source)?;
        match hash_type {
            HashType::Sha512 => {
                update_digest_from_reader(&mut file, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                update_digest_from_reader(&mut file, &mut hasher_sha256)?;
            }
        }
    };

    let hash = match hash_type {
        HashType::Sha512 => hex::encode(hasher_sha512.finalize()),
        HashType::Sha256 => hex::encode(hasher_sha256.finalize()),
    };

    Ok(hash)
}

pub mod validate {
    use anyhow::{Result, anyhow};
    use colored::Colorize;
    use std::path::Path;

    pub fn run(file: &Path) -> Result<()> {
        if !file.exists() {
            return Err(anyhow!("File does not exist: {}", file.display()));
        }

        let content = std::fs::read_to_string(file)?;
        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");

        println!("{} Validating {}...", "::".bold().blue(), file.display());

        if file_name == "registries.json" {
            let _: crate::pkg::purl::CentralDbSpec = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid registries.json spec: {}", e))?;
            println!(
                "{} file is a valid registries.json spec.",
                "OK".bold().green()
            );
        } else if file_name == "repo.yaml" || file_name == "repo.yml" {
            let _: crate::pkg::types::RepoConfig = serde_yaml::from_str(&content)
                .map_err(|e| anyhow!("Invalid repo.yaml spec: {}", e))?;
            println!("{} file is a valid repo.yaml spec.", "OK".bold().green());
        } else if file_name == "advisories.json" {
            let _: crate::pkg::types::AdvisoryRegistry = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid advisories.json spec: {}", e))?;
            println!(
                "{} file is a valid advisories.json spec.",
                "OK".bold().green()
            );
        } else if file_name == "packages.json" {
            let _: crate::pkg::purl::RegistryIndex = serde_json::from_str(&content)
                .map_err(|e| anyhow!("Invalid packages.json spec: {}", e))?;
            println!(
                "{} file is a valid packages.json spec.",
                "OK".bold().green()
            );
        } else if file_name.ends_with(".sec.yaml") || file_name.ends_with(".sec.yml") {
            let _: crate::pkg::types::Advisory = serde_yaml::from_str(&content)
                .map_err(|e| anyhow!("Invalid security advisory (.sec.yaml) spec: {}", e))?;
            println!("{} file is a valid .sec.yaml spec.", "OK".bold().green());
        } else {
            if file.extension().and_then(|e| e.to_str()) == Some("json") {
                if serde_json::from_str::<crate::pkg::purl::CentralDbSpec>(&content).is_ok() {
                    println!("{} file matches registries.json spec.", "OK".bold().green());
                } else if serde_json::from_str::<crate::pkg::types::AdvisoryRegistry>(&content)
                    .is_ok()
                {
                    println!("{} file matches advisories.json spec.", "OK".bold().green());
                } else if serde_json::from_str::<crate::pkg::purl::RegistryIndex>(&content).is_ok()
                {
                    println!("{} file matches packages.json spec.", "OK".bold().green());
                } else {
                    return Err(anyhow!(
                        "File does not match any known Zoi JSON spec (registries.json, advisories.json, or packages.json)"
                    ));
                }
            } else if file.extension().and_then(|e| e.to_str()) == Some("yaml")
                || file.extension().and_then(|e| e.to_str()) == Some("yml")
            {
                if serde_yaml::from_str::<crate::pkg::types::RepoConfig>(&content).is_ok() {
                    println!("{} file matches repo.yaml spec.", "OK".bold().green());
                } else if serde_yaml::from_str::<crate::pkg::types::Advisory>(&content).is_ok() {
                    println!("{} file matches .sec.yaml spec.", "OK".bold().green());
                } else {
                    return Err(anyhow!(
                        "File does not match any known Zoi YAML spec (repo.yaml or .sec.yaml)"
                    ));
                }
            } else {
                return Err(anyhow!(
                    "Unsupported file extension. Please provide a .json or .yaml file"
                ));
            }
        }

        Ok(())
    }
}
