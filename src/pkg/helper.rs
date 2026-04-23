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
