use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString},
};
use base64::{Engine as _, engine::general_purpose};
use rand::Rng; // Former RngCore
use rand::rng;
use std::fs; // Former thread_rng

const SECRET_PREFIX: &str = "ZOISEC:v1:";

/// Generates a one-way password hash using Argon2 (modern standard).
pub fn hash_password(password: &str) -> Result<String> {
    let mut salt_bytes = [0u8; 16];
    rng().fill_bytes(&mut salt_bytes);
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow!("Failed to encode salt: {}", e))?;

    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash password: {}", e))?
        .to_string();
    Ok(password_hash)
}

/// Retrieves or generates the local master key for two-way encryption.
fn get_master_key() -> Result<[u8; 32]> {
    let mut key_path = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    key_path.push(".zoi/master.key");

    if !key_path.exists() {
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut key = [0u8; 32];
        rng().fill_bytes(&mut key);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&key_path)?;
            use std::io::Write;
            file.write_all(&key)?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&key_path, key)?;
        }

        return Ok(key);
    }

    let key_bytes = fs::read(&key_path)?;
    if key_bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid master key length at {}",
            key_path.display()
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);
    Ok(key)
}

/// Encrypts a string so only this Zoi installation can decrypt it.
pub fn encrypt_secret(plaintext: &str) -> Result<String> {
    let key_bytes = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)?;

    let mut nonce_bytes = [0u8; 12];
    rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::try_from(&nonce_bytes[..]).map_err(|e| anyhow!("Invalid nonce: {}", e))?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    let mut combined = Vec::new();
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encoded = general_purpose::STANDARD.encode(combined);
    Ok(format!("{}{}", SECRET_PREFIX, encoded))
}

/// Decrypts a ZOISEC string. Returns the original plaintext or the input if it's not a secret.
pub fn decrypt_secret(input: &str) -> Result<String> {
    if !input.starts_with(SECRET_PREFIX) {
        return Ok(input.to_string());
    }

    let encoded = &input[SECRET_PREFIX.len()..];
    let combined = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| anyhow!("Failed to decode base64 secret: {}", e))?;

    if combined.len() < 12 {
        return Err(anyhow!("Invalid secret length"));
    }

    let key_bytes = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)?;

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce =
        Nonce::try_from(nonce_bytes).map_err(|e| anyhow!("Invalid nonce in secret: {}", e))?;

    let plaintext_bytes = cipher.decrypt(&nonce, ciphertext).map_err(|e| {
        anyhow!(
            "Decryption failed: {}. This secret might have been encrypted on a different machine.",
            e
        )
    })?;

    let plaintext = String::from_utf8(plaintext_bytes)?;
    Ok(plaintext)
}
