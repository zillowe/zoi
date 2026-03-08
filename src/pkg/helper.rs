use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;

pub enum HashType {
    Sha512,
    Sha256,
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
                std::io::copy(&mut response, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                std::io::copy(&mut response, &mut hasher_sha256)?;
            }
        }
    } else {
        let mut file = File::open(source)?;
        match hash_type {
            HashType::Sha512 => {
                std::io::copy(&mut file, &mut hasher_sha512)?;
            }
            HashType::Sha256 => {
                std::io::copy(&mut file, &mut hasher_sha256)?;
            }
        }
    };

    let hash = match hash_type {
        HashType::Sha512 => hex::encode(hasher_sha512.finalize()),
        HashType::Sha256 => hex::encode(hasher_sha256.finalize()),
    };

    Ok(hash)
}
