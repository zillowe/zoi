use std::fs;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Result, anyhow};
use argon2::Argon2;
use argon2::password_hash::PasswordHasher;
use base64::Engine as _;
use base64::engine::general_purpose;
use rand::{Rng, rng};
use zoi_core::utils::get_user_state_dir;

/// Secret prefix.
const SECRET_PREFIX: &str = "ZOISEC:v1:";

/// The algorithms `zoi system secret hash` can hash a password with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashAlgorithm {
    /// yescrypt (`$y$...`), the default: what Arch, Debian and Fedora use for
    /// new passwords, and the strongest hash libxcrypt (and so PAM, `login`,
    /// `su`, `sudo` and sshd) can verify.
    #[default]
    Yescrypt,
    /// Argon2id (`$argon2id$...`) in PHC string format. libxcrypt cannot
    /// verify it, so it does not work as a login password in `/etc/shadow`;
    /// it is for programs that check Argon2 themselves.
    Argon2
}

/// Generates a one-way password hash with the default algorithm (yescrypt),
/// in the form `/etc/shadow` and `users({...})` in system.lua expect.
///
/// # Errors
///
/// Returns an error if the hashing parameters are rejected.
pub fn hash_password(password: &str) -> Result<String> {
    hash_password_with(password, HashAlgorithm::default())
}

/// Generates a one-way password hash with `algorithm`, using a random salt
/// and the algorithm's recommended parameters (for yescrypt the same as
/// libxcrypt's default, `$y$j9T$...`).
///
/// # Errors
///
/// Returns an error if the hashing parameters are rejected.
pub fn hash_password_with(
    password: &str,
    algorithm: HashAlgorithm
) -> Result<String> {
    let hash = match algorithm {
        HashAlgorithm::Yescrypt => yescrypt::Yescrypt::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.as_str().to_string()),
        HashAlgorithm::Argon2 => Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
    };
    hash.map_err(|e| anyhow!("Failed to hash password: {e}"))
}

/// Retrieves or generates the local master key for two-way encryption.
fn get_master_key() -> Result<[u8; 32]> {
    let key_path = get_user_state_dir()?.join("master.key");

    if !key_path.exists() {
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut key = [0u8; 32];
        rng().fill_bytes(&mut key);

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&key_path)?;
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

/// Exports the master key as a base64-encoded string.
///
/// # Errors
///
/// Returns an error if no master key has been generated yet, or if it
/// cannot be read.
pub fn export_master_key() -> Result<String> {
    let key = get_master_key()?;
    Ok(general_purpose::STANDARD.encode(key))
}

/// Imports a base64-encoded master key.
///
/// # Errors
///
/// Returns an error if `encoded_key` is not valid base64, or if the key
/// file cannot be written.
pub fn import_master_key(encoded_key: &str) -> Result<()> {
    let key_bytes = general_purpose::STANDARD
        .decode(encoded_key)
        .map_err(|e| anyhow!("Failed to decode base64 key: {e}"))?;

    if key_bytes.len() != 32 {
        return Err(anyhow!("Invalid master key length. Expected 32 bytes."));
    }

    let key_path = get_user_state_dir()?.join("master.key");

    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&key_path)?;
        file.write_all(&key_bytes)?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&key_path, key_bytes)?;
    }

    Ok(())
}

/// Encrypts a string so only this Zoi installation can decrypt it.
///
/// # Errors
///
/// Returns an error if no master key has been generated yet, or if
/// encryption fails.
pub fn encrypt_secret(plaintext: &str) -> Result<String> {
    let key_bytes = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)?;

    let mut nonce_bytes = [0u8; 12];
    rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::try_from(&nonce_bytes[..])
        .map_err(|e| anyhow!("Invalid nonce: {e}"))?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {e}"))?;

    let mut combined = Vec::new();
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encoded = general_purpose::STANDARD.encode(combined);
    Ok(format!("{SECRET_PREFIX}{encoded}"))
}

/// Decrypts a ZOISEC string. Returns the original plaintext or the input if
/// it's not a secret.
///
/// # Errors
///
/// Returns an error if the input carries the secret prefix but cannot be
/// decrypted, which means the master key has changed.
pub fn decrypt_secret(input: &str) -> Result<String> {
    if !input.starts_with(SECRET_PREFIX) {
        return Ok(input.to_string());
    }

    let encoded = &input[SECRET_PREFIX.len()..];
    let combined = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| anyhow!("Failed to decode base64 secret: {e}"))?;

    if combined.len() < 12 {
        return Err(anyhow!("Invalid secret length"));
    }

    let key_bytes = get_master_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)?;

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::try_from(nonce_bytes)
        .map_err(|e| anyhow!("Invalid nonce in secret: {e}"))?;

    let plaintext_bytes = cipher.decrypt(&nonce, ciphertext).map_err(|e| {
        anyhow!(
            "Decryption failed: {e}. This secret might have been encrypted on \
             a different machine."
        )
    })?;

    let plaintext = String::from_utf8(plaintext_bytes)?;
    Ok(plaintext)
}
