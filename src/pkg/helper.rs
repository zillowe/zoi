use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fs::File;
use std::io::Read;

pub enum HashType {
    Sha512,
    Sha256,
}

pub fn get_hash(source: &str, hash_type: HashType) -> Result<String, Box<dyn Error>> {
    let bytes: Vec<u8> = if source.starts_with("http://") || source.starts_with("https://") {
        let client = crate::utils::build_blocking_http_client(60)?;
        let response = client.get(source).send()?;
        if !response.status().is_success() {
            return Err(format!("Failed to download file from URL: {}", response.status()).into());
        }
        response.bytes()?.to_vec()
    } else {
        let mut file = File::open(source)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        buffer
    };

    let hash = match hash_type {
        HashType::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        HashType::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
    };

    Ok(hash)
}
