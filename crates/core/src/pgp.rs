use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use colored::Colorize;
use sequoia_openpgp::Cert;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::types::RevocationStatus;

// Manages PGP keys and signature verification for the Zoi "Chain of Trust".
//
// Zoi uses PGP to verify:
// - Registry Integrity: Every git commit in an official registry should be
//   signed.
// - Package Authenticity: Pre-built archives are verified against maintainer
//   keys.
//
// This module handles local keyring management in Zoi's user data directory and
// provides utilities for importing, searching, and verifying signatures.

include!(concat!(env!("OUT_DIR"), "/generated_pgp_keys.rs"));

/// Synchronizes the local keyring with trusted keys embedded in the Zoi binary.
///
/// During the build process, Zoi bakes in "Root of Trust" keys for official
/// registries. This function ensures these keys are present and up-to-date
/// in the user's local keyring on every startup.
///
/// # Errors
///
/// Returns an error if the local keyring directory cannot be created or
/// accessed.
pub fn ensure_builtin_keys() -> Result<()> {
    for (name, bytes) in BUILTIN_KEYS {
        if let Err(e) = add_key_from_bytes(bytes, name, true) {
            eprintln!(
                "Warning: Failed to ensure builtin PGP key '{name}': {e}"
            );
        }
    }
    Ok(())
}

/// Returns a human-readable string representing the status of a PGP
/// certificate.
///
/// The status can be "Valid", "Revoked", "Expired", or "Invalid" with
/// additional details like expiration dates where applicable.
pub fn get_cert_status(cert: &Cert) -> String {
    let policy = StandardPolicy::new();
    let now = SystemTime::now();
    match cert.with_policy(&policy, now) {
        Ok(vc) => {
            if let RevocationStatus::Revoked(_) = vc.revocation_status() {
                return "Revoked".red().bold().to_string();
            }
            if let Some(expiration) = vc.primary_key().key_expiration_time() {
                let datetime: DateTime<Utc> = DateTime::<Utc>::from(expiration);
                if expiration < now {
                    return format!(
                        "Expired ({})",
                        datetime.format("%Y-%m-%d")
                    )
                    .red()
                    .to_string();
                }
                return format!(
                    "Valid (expires {})",
                    datetime.format("%Y-%m-%d")
                )
                .green()
                .to_string();
            }
            "Valid (no expiration)".green().to_string()
        }
        Err(e) => format!("Invalid: {e}").red().to_string()
    }
}

/// Validates a PGP certificate, checking for revocation and expiration.
///
/// Returns `Ok(())` if the certificate is valid, or an error if it is revoked,
/// expired, or otherwise invalid under the standard policy.
///
/// # Errors
///
/// Returns an error if the certificate is revoked, expired, or invalid.
pub fn validate_cert(cert: &Cert) -> Result<()> {
    let policy = StandardPolicy::new();
    let now = SystemTime::now();
    match cert.with_policy(&policy, now) {
        Ok(vc) => {
            if let RevocationStatus::Revoked(_) = vc.revocation_status() {
                return Err(anyhow!("The PGP key is revoked."));
            }
            if let Some(expiration) = vc.primary_key().key_expiration_time()
                && expiration < now
            {
                let datetime: DateTime<Utc> = DateTime::<Utc>::from(expiration);
                return Err(anyhow!(
                    "The PGP key expired on {}.",
                    datetime.format("%Y-%m-%d")
                ));
            }
            Ok(())
        }
        Err(e) => Err(anyhow!("The PGP key is invalid: {e}"))
    }
}

/// Returns the path to the local PGP keyring directory.
///
/// This resides in Zoi's user data directory under `pgps/`. The directory is
/// created if it does not exist.
///
/// # Errors
///
/// Returns an error if the home directory cannot be found or the PGP directory
/// cannot be created.
pub fn get_pgp_dir() -> Result<PathBuf> {
    let pgp_dir = crate::utils::get_user_data_dir()?.join("pgps");
    fs::create_dir_all(&pgp_dir)?;
    Ok(pgp_dir)
}

/// Adds a PGP key from a byte slice to the local keyring.
///
/// The key is saved as `<name>.asc` in the PGP directory. If a key with the
/// same name exists but has different content, it will be overwritten (with a
/// warning if `quiet` is false).
///
/// # Errors
///
/// Returns an error if the PGP directory cannot be accessed, or if the key is
/// invalid.
pub fn add_key_from_bytes(
    key_bytes: &[u8],
    name: &str,
    quiet: bool
) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let dest_path = pgp_dir.join(format!("{name}.asc"));

    if dest_path.exists() {
        let existing_bytes = fs::read(&dest_path)?;
        if existing_bytes == key_bytes {
            return Ok(());
        }
        if !quiet {
            println!(
                "{} A different key with the name '{name}' already exists. \
                 Overwriting.",
                "Warning:".yellow(),
            );
        }
    }

    let cert = Cert::from_bytes(key_bytes)?;
    validate_cert(&cert)?;

    fs::write(&dest_path, key_bytes)?;
    if !quiet {
        println!("Successfully added/updated key '{}'.", name.cyan());
    }

    Ok(())
}

/// Imports a PGP key from a file path into the local keyring.
///
/// If `name` is provided, it is used as the key's name in the store.
/// Otherwise, the file stem of the path is used.
///
/// # Errors
///
/// Returns an error if the key file does not exist or cannot be read.
pub fn add_key_from_path(
    path: &str,
    name: Option<&str>,
    quiet: bool
) -> Result<()> {
    let key_path = Path::new(path);
    if !key_path.exists() {
        return Err(anyhow!("Key file not found at: {path}"));
    }

    let key_name = name.unwrap_or_else(|| {
        key_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
    });

    if !quiet {
        println!("Validating PGP key file...");
    }
    let key_bytes = fs::read(key_path)?;
    if !quiet {
        println!("{}", "Key is valid.".green());
    }

    add_key_from_bytes(&key_bytes, key_name, quiet)
}

/// Fetches a PGP key from a keyserver by fingerprint and adds it to the local
/// keyring.
///
/// Currently uses `keys.openpgp.org` as the keyserver.
///
/// # Errors
///
/// Returns an error if the key cannot be fetched from the keyserver or is
/// invalid.
pub fn add_key_from_fingerprint(
    fingerprint: &str,
    name: &str,
    quiet: bool
) -> Result<()> {
    let url = format!(
        "https://keys.openpgp.org/vks/v1/by-fingerprint/{}",
        fingerprint.to_uppercase()
    );
    if !quiet {
        println!(
            "Fetching key for fingerprint {} from keys.openpgp.org...",
            fingerprint.cyan()
        );
    }

    let client = crate::utils::get_http_client()?;
    let response = client.get(&url).send()?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch key from keyserver (HTTP {}).",
            response.status()
        ));
    }

    let key_bytes = response.bytes()?.to_vec();

    if !quiet {
        println!("Validating PGP key...");
    }
    Cert::from_bytes(&key_bytes)?;
    if !quiet {
        println!("{}", "Key is valid.".green());
    }

    add_key_from_bytes(&key_bytes, name, quiet)
}

/// Downloads a PGP key from a URL and adds it to the local keyring.
///
/// # Errors
///
/// Returns an error if the key cannot be fetched from the URL or is invalid.
pub fn add_key_from_url(url: &str, name: &str, quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "Fetching key for {} from url {}...",
            name.cyan(),
            url.cyan()
        );
    }

    let client = crate::utils::get_http_client()?;
    let response = client.get(url).send()?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch key from url (HTTP {})",
            response.status()
        ));
    }

    let key_bytes = response.bytes()?.to_vec();

    if !quiet {
        println!("Validating PGP key...");
    }
    Cert::from_bytes(&key_bytes)?;
    if !quiet {
        println!("{}", "Key is valid.".green());
    }

    add_key_from_bytes(&key_bytes, name, quiet)
}

/// Removes a PGP key from the local keyring by its name.
///
/// # Errors
///
/// Returns an error if the key with the given name is not found or cannot be
/// removed.
pub fn remove_key_by_name(name: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{name}.asc"));

    if !key_path.exists() {
        return Err(anyhow!("Key with name '{name}' not found."));
    }

    fs::remove_file(&key_path)?;
    println!("Successfully removed key '{}'.", name.cyan());

    Ok(())
}

/// Searches for and removes a PGP key from the local keyring by its
/// fingerprint.
///
/// # Errors
///
/// Returns an error if no key with the given fingerprint is found or cannot be
/// removed.
pub fn remove_key_by_fingerprint(fingerprint: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let fingerprint_upper = fingerprint.to_uppercase();

    for entry in fs::read_dir(pgp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("asc")
        {
            let key_bytes = fs::read(&path)?;
            if let Ok(cert) = Cert::from_bytes(&key_bytes)
                && cert.fingerprint().to_string().to_uppercase()
                    == fingerprint_upper
            {
                fs::remove_file(&path)?;
                println!(
                    "Successfully removed key with fingerprint {}.",
                    fingerprint.cyan()
                );
                return Ok(());
            }
        }
    }

    Err(anyhow!("Key with fingerprint '{fingerprint}' not found."))
}

/// Prints a formatted list of all PGP keys stored in the local keyring.
///
/// # Errors
///
/// Returns an error if the local keyring cannot be read.
pub fn list_keys() -> Result<()> {
    let keys = get_all_local_keys_info()?;

    if keys.is_empty() {
        println!("No PGP keys found in the store.");
        return Ok(());
    }

    println!("{} Stored PGP Keys", "::".bold().blue());

    for key_info in keys {
        println!();
        println!("{}: {}", "Name".cyan(), key_info.name.bold());
        println!("{}: {}", "  Status".cyan(), get_cert_status(&key_info.cert));
        println!(
            "  {}: {}",
            "Fingerprint".cyan(),
            key_info.cert.fingerprint()
        );
        for userid_amalgamation in key_info.cert.userids() {
            let userid_packet = userid_amalgamation.userid();
            let name = userid_packet
                .name()
                .ok()
                .flatten()
                .unwrap_or("[invalid name]");
            let email =
                userid_packet.email().ok().flatten().unwrap_or_default();

            if email.is_empty() {
                println!("  {}: {}", "UserID".cyan(), name);
            } else {
                println!("  {}: {} <{}>", "UserID".cyan(), name, email);
            }
        }
    }

    Ok(())
}

/// Searches for PGP keys in the local keyring by name, fingerprint, or `UserID`
/// (name/email).
///
/// # Errors
///
/// Returns an error if the local keyring cannot be read.
pub fn search_keys(term: &str) -> Result<()> {
    let keys = get_all_local_keys_info()?;
    let term_lower = term.to_lowercase();
    let mut found_keys = Vec::new();

    for key_info in keys {
        let fingerprint =
            key_info.cert.fingerprint().to_string().to_lowercase();
        let name = key_info.name.to_lowercase();

        let mut is_match =
            name.contains(&term_lower) || fingerprint.contains(&term_lower);

        if !is_match {
            for userid_amalgamation in key_info.cert.userids() {
                let userid_packet = userid_amalgamation.userid();
                let uid_name = userid_packet
                    .name()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_lowercase();
                let uid_email = userid_packet
                    .email()
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .to_lowercase();

                if uid_name.contains(&term_lower)
                    || uid_email.contains(&term_lower)
                {
                    is_match = true;
                    break;
                }
            }
        }

        if is_match {
            found_keys.push(key_info);
        }
    }

    if found_keys.is_empty() {
        println!("\n{}", "No keys found matching your query.".yellow());
        return Ok(());
    }

    println!(
        "{} Found {} key(s) matching '{}'",
        "::".bold().blue(),
        found_keys.len(),
        term.blue().bold()
    );

    for key_info in found_keys {
        println!();
        println!("{}: {}", "Name".cyan(), key_info.name.bold());
        println!("{}: {}", "  Status".cyan(), get_cert_status(&key_info.cert));
        println!(
            "  {}: {}",
            "Fingerprint".cyan(),
            key_info.cert.fingerprint()
        );
        for userid_amalgamation in key_info.cert.userids() {
            let userid_packet = userid_amalgamation.userid();
            let name = userid_packet
                .name()
                .ok()
                .flatten()
                .unwrap_or("[invalid name]");
            let email =
                userid_packet.email().ok().flatten().unwrap_or_default();

            if email.is_empty() {
                println!("  {}: {}", "UserID".cyan(), name);
            } else {
                println!("  {}: {} <{}>", "UserID".cyan(), name, email);
            }
        }
    }

    Ok(())
}

/// Prints the ASCII-armored content of a PGP key from the local keyring.
///
/// # Errors
///
/// Returns an error if the key with the given name is not found or cannot be
/// read.
pub fn show_key(name: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{name}.asc"));

    if !key_path.exists() {
        return Err(anyhow!("Key with name '{name}' not found."));
    }

    let key_contents = fs::read_to_string(&key_path)?;
    println!("{key_contents}");

    Ok(())
}

/// A structure holding a PGP key's name and its parsed certificate.
pub struct KeyInfo {
    /// The name of the key (usually its filename without extension).
    pub name: String,
    /// The parsed PGP certificate.
    pub cert: Cert
}

/// Retrieves information for all PGP keys stored in the local keyring.
///
/// # Errors
///
/// Returns an error if the local keyring cannot be read or contains invalid
/// keys.
pub fn get_all_local_keys_info() -> Result<Vec<KeyInfo>> {
    let pgp_dir = get_pgp_dir()?;
    let mut keys = Vec::new();
    if !pgp_dir.exists() {
        return Ok(keys);
    }
    for entry in fs::read_dir(pgp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("asc")
            && let Ok(bytes) = fs::read(&path)
            && let Ok(cert) = Cert::from_bytes(&bytes)
        {
            let name = path
                .file_stem()
                .ok_or_else(|| {
                    anyhow!("Path should have a file stem: {}", path.display())
                })?
                .to_string_lossy()
                .to_string();
            keys.push(KeyInfo { name, cert });
        }
    }
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(keys)
}

/// Retrieves all PGP certificates stored in the local keyring.
///
/// # Errors
///
/// Returns an error if the local keyring cannot be read or contains invalid
/// keys.
pub fn get_all_local_certs() -> Result<Vec<Cert>> {
    let pgp_dir = get_pgp_dir()?;
    let mut certs = Vec::new();
    if !pgp_dir.exists() {
        return Ok(certs);
    }
    for entry in fs::read_dir(pgp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("asc")
            && let Ok(bytes) = fs::read(&path)
            && let Ok(cert) = Cert::from_bytes(&bytes)
        {
            certs.push(cert);
        }
    }
    Ok(certs)
}

use sequoia_openpgp::KeyHandle;
use sequoia_openpgp::parse::stream::{
    DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper
};

/// A Sequoia verification helper that allows verification against multiple
/// trusted certificates.
struct MultiCertHelper {
    /// The list of trusted certificates to use for verification.
    certs: Vec<Cert>
}

impl VerificationHelper for MultiCertHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        Ok(self.certs.clone())
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(Result::is_ok) {
                        return Ok(());
                    }
                    return Err(anyhow!(
                        "No valid signature found from any trusted key."
                    ));
                }
                _ => {
                    return Err(anyhow!(
                        "Unexpected message structure: not a signature group."
                    ));
                }
            }
        }
        Err(anyhow!(
            "No signature layer found in the message structure."
        ))
    }
}

/// A Sequoia verification helper that allows verification against a single
/// trusted certificate.
struct OneCertHelper {
    /// The trusted certificate to use for verification.
    cert: Cert
}

impl VerificationHelper for OneCertHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(Result::is_ok) {
                        return Ok(());
                    }
                    return Err(anyhow!("No valid signature found"));
                }
                _ => return Err(anyhow!("Unexpected message structure"))
            }
        }
        Err(anyhow!("No signature layer found"))
    }
}

/// A CLI-friendly wrapper for verifying a file's signature using a named key
/// from the local keyring.
///
/// # Errors
///
/// Returns an error if the key is not found or the signature is invalid.
pub fn cli_verify_signature(
    file_path: &str,
    sig_path: &str,
    key_name: &str
) -> Result<()> {
    println!(
        "Verifying {file_path} with signature {sig_path} using key \
         '{key_name}'"
    );

    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{key_name}.asc"));
    if !key_path.exists() {
        return Err(anyhow!("Key '{key_name}' not found in local store."));
    }
    let key_bytes = fs::read(key_path)?;
    let cert = Cert::from_bytes(&key_bytes)?;

    verify_detached_signature(
        Path::new(file_path),
        Path::new(sig_path),
        &cert
    )?;

    println!("{}", "Signature is valid.".green());
    Ok(())
}

/// Verifies a detached PGP signature for a file using a specific certificate.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the signature is invalid.
pub fn verify_detached_signature(
    data_path: &Path,
    signature_path: &Path,
    cert: &Cert
) -> Result<()> {
    let data = fs::read(data_path)?;
    let signature = fs::read(signature_path)?;
    verify_detached_signature_raw(&data, &signature, cert)
}

/// Verifies a detached PGP signature for raw data using a specific certificate.
///
/// # Errors
///
/// Returns an error if the signature is invalid.
pub fn verify_detached_signature_raw(
    data: &[u8],
    signature: &[u8],
    cert: &Cert
) -> Result<()> {
    let policy = &StandardPolicy::new();
    let helper = OneCertHelper { cert: cert.clone() };

    let mut verifier = DetachedVerifierBuilder::from_bytes(signature)?
        .with_policy(policy, None, helper)?;

    verifier.verify_bytes(data)?;

    Ok(())
}

/// Signs a file using `GnuPG`.
///
/// This function calls the external `gpg` command to create a detached
/// signature. It can take an optional `GPG_PASSWORD` environment variable for
/// password-protected keys.
///
/// # Errors
///
/// Returns an error if `gpg` is not installed, the file cannot be read, or
/// signing fails.
pub fn sign_detached(
    data_path: &Path,
    signature_path: &Path,
    key_id: &str
) -> Result<()> {
    if !crate::utils::command_exists("gpg") {
        return Err(anyhow!(
            "gpg command not found. Please install GnuPG and ensure it's in \
             your PATH."
        ));
    }

    let data_path_str = data_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid data path for signing."))?;
    let signature_path_str = signature_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid signature path for signing."))?;

    let mut command = Command::new("gpg");
    command
        .arg("--batch")
        .arg("--no-tty")
        .arg("--yes")
        .arg("--detach-sign");

    if let Ok(password) = std::env::var("GPG_PASSWORD") {
        command
            .arg("--pinentry-mode")
            .arg("loopback")
            .arg("--passphrase")
            .arg(password);
    }

    command
        .arg("--local-user")
        .arg(key_id)
        .arg("--output")
        .arg(signature_path_str)
        .arg(data_path_str);

    let output = command.output()?;

    if !output.status.success() {
        use std::fmt::Write;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut error_message =
            format!("gpg signing failed with status: {}.\n", output.status);
        if stderr.contains("No secret key") {
            let _ = writeln!(
                error_message,
                "The secret key for '{key_id}' was not found in your GPG \
                 keychain."
            );
            error_message.push_str(
                "Please ensure the key is imported into GPG and is trusted."
            );
        } else if stderr.contains("bad passphrase")
            || stderr.contains("Passphrase check failed")
        {
            error_message.push_str(
                "Incorrect passphrase provided, or the agent could not get \
                 the passphrase.\n"
            );
            error_message.push_str(
                "Ensure your GPG agent is running and configured correctly if \
                 the key is password-protected."
            );
        } else {
            let _ = write!(error_message, "Stderr: {stderr}");
        }

        return Err(anyhow!(error_message));
    }

    Ok(())
}

/// Resolves a list of names or fingerprints to PGP certificates from the local
/// keyring.
///
/// # Errors
///
/// Returns an error if any of the trusted keys are not found in the local
/// keyring.
pub fn get_certs_by_name_or_fingerprint(
    identifiers: &[String]
) -> Result<Vec<Cert>> {
    let all_keys = get_all_local_keys_info()?;
    let mut found_certs = Vec::new();

    for identifier in identifiers {
        let identifier_lower = identifier.to_lowercase();
        let mut found = false;
        for key_info in &all_keys {
            let fingerprint_lower =
                key_info.cert.fingerprint().to_string().to_lowercase();
            if key_info.name == *identifier
                || fingerprint_lower.starts_with(&identifier_lower)
            {
                found_certs.push(key_info.cert.clone());
                found = true;
                break;
            }
        }
        if !found {
            return Err(anyhow!(
                "Trusted key '{identifier}' not found in Zoi's PGP keyring."
            ));
        }
    }
    Ok(found_certs)
}

/// Verifies a detached PGP signature for a file against a set of trusted
/// certificates.
///
/// Returns `Ok(())` if at least one trusted certificate successfully verifies
/// the signature.
///
/// # Errors
///
/// Returns an error if the file cannot be read or no valid signature is found.
pub fn verify_detached_signature_multi_key(
    data_path: &Path,
    signature_path: &Path,
    trusted_certs: Vec<Cert>
) -> Result<()> {
    let policy = &StandardPolicy::new();
    let data = fs::read(data_path)?;
    let signature = fs::read(signature_path)?;

    let helper = MultiCertHelper {
        certs: trusted_certs
    };

    let mut verifier = DetachedVerifierBuilder::from_bytes(&signature)?
        .with_policy(policy, None, helper)?;

    verifier.verify_bytes(&data)?;

    Ok(())
}
