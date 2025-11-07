use anyhow::{Result, anyhow};
use colored::*;
use sequoia_openpgp::Cert;
use sequoia_openpgp::parse::Parse;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn get_pgp_dir() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| anyhow!("Could not find home directory."))?;
    let pgp_dir = home_dir.join(".zoi").join("pgps");
    fs::create_dir_all(&pgp_dir)?;
    Ok(pgp_dir)
}

pub fn add_key_from_bytes(key_bytes: &[u8], name: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let dest_path = pgp_dir.join(format!("{}.asc", name));

    if dest_path.exists() {
        let existing_bytes = fs::read(&dest_path)?;
        if existing_bytes == key_bytes {
            return Ok(());
        }
        println!(
            "{} A different key with the name '{}' already exists. Overwriting.",
            "Warning:".yellow(),
            name
        );
    }

    Cert::from_bytes(key_bytes)?;

    fs::write(&dest_path, key_bytes)?;
    println!("Successfully added/updated key '{}'.", name.cyan());

    Ok(())
}

pub fn add_key_from_path(path: &str, name: Option<&str>) -> Result<()> {
    let key_path = Path::new(path);
    if !key_path.exists() {
        return Err(anyhow!("Key file not found at: {}", path));
    }

    let key_name = name.unwrap_or_else(|| {
        key_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
    });

    println!("Validating PGP key file...");
    let key_bytes = fs::read(key_path)?;
    println!("{}", "Key is valid.".green());

    add_key_from_bytes(&key_bytes, key_name)
}

pub fn add_key_from_fingerprint(fingerprint: &str, name: &str) -> Result<()> {
    let url = format!(
        "https://keys.openpgp.org/vks/v1/by-fingerprint/{}",
        fingerprint.to_uppercase()
    );
    println!(
        "Fetching key for fingerprint {} from keys.openpgp.org...",
        fingerprint.cyan()
    );

    let response = reqwest::blocking::get(&url)?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch key from keyserver (HTTP {}).",
            response.status()
        ));
    }

    let key_bytes = response.bytes()?.to_vec();

    println!("Validating PGP key...");
    Cert::from_bytes(&key_bytes)?;
    println!("{}", "Key is valid.".green());

    add_key_from_bytes(&key_bytes, name)
}

pub fn add_key_from_url(url: &str, name: &str) -> Result<()> {
    println!(
        "Fetching key for {} from url {}...",
        name.cyan(),
        url.cyan()
    );

    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch key from url (HTTP {})",
            response.status()
        ));
    }

    let key_bytes = response.bytes()?.to_vec();

    println!("Validating PGP key...");
    Cert::from_bytes(&key_bytes)?;
    println!("{}", "Key is valid.".green());

    add_key_from_bytes(&key_bytes, name)
}

pub fn remove_key_by_name(name: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{}.asc", name));

    if !key_path.exists() {
        return Err(anyhow!("Key with name '{}' not found.", name));
    }

    fs::remove_file(&key_path)?;
    println!("Successfully removed key '{}'.", name.cyan());

    Ok(())
}

pub fn remove_key_by_fingerprint(fingerprint: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let fingerprint_upper = fingerprint.to_uppercase();

    for entry in fs::read_dir(pgp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("asc") {
            let key_bytes = fs::read(&path)?;
            if let Ok(cert) = Cert::from_bytes(&key_bytes)
                && cert.fingerprint().to_string().to_uppercase() == fingerprint_upper
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

    Err(anyhow!("Key with fingerprint '{}' not found.", fingerprint))
}

pub fn list_keys() -> Result<()> {
    let keys = get_all_local_keys_info()?;

    if keys.is_empty() {
        println!("No PGP keys found in the store.");
        return Ok(());
    }

    println!("{}", "--- Stored PGP Keys ---".yellow().bold());

    for key_info in keys {
        println!();
        println!("{}: {}", "Name".cyan(), key_info.name.bold());
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
            let email = userid_packet.email().ok().flatten().unwrap_or_default();

            if !email.is_empty() {
                println!("  {}: {} <{}>", "UserID".cyan(), name, email);
            } else {
                println!("  {}: {}", "UserID".cyan(), name);
            }
        }
    }

    Ok(())
}

pub fn search_keys(term: &str) -> Result<()> {
    let keys = get_all_local_keys_info()?;
    let term_lower = term.to_lowercase();
    let mut found_keys = Vec::new();

    for key_info in keys {
        let fingerprint = key_info.cert.fingerprint().to_string().to_lowercase();
        let name = key_info.name.to_lowercase();

        let mut is_match = name.contains(&term_lower) || fingerprint.contains(&term_lower);

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

                if uid_name.contains(&term_lower) || uid_email.contains(&term_lower) {
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
        "--- Found {} key(s) matching '{}' ---",
        found_keys.len(),
        term.blue().bold()
    );

    for key_info in found_keys {
        println!();
        println!("{}: {}", "Name".cyan(), key_info.name.bold());
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
            let email = userid_packet.email().ok().flatten().unwrap_or_default();

            if !email.is_empty() {
                println!("  {}: {} <{}>", "UserID".cyan(), name, email);
            } else {
                println!("  {}: {}", "UserID".cyan(), name);
            }
        }
    }

    Ok(())
}

pub fn show_key(name: &str) -> Result<()> {
    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{}.asc", name));

    if !key_path.exists() {
        return Err(anyhow!("Key with name '{}' not found.", name));
    }

    let key_contents = fs::read_to_string(&key_path)?;
    println!("{}", key_contents);

    Ok(())
}

pub struct KeyInfo {
    pub name: String,
    pub cert: Cert,
}

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
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            keys.push(KeyInfo { name, cert });
        }
    }
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(keys)
}

use sequoia_openpgp::policy::StandardPolicy;

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

use sequoia_openpgp::{
    KeyHandle,
    parse::stream::{DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper},
};

struct MultiCertHelper {
    certs: Vec<Cert>,
}

impl VerificationHelper for MultiCertHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        Ok(self.certs.clone())
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(|r| r.is_ok()) {
                        return Ok(());
                    } else {
                        return Err(anyhow!("No valid signature found from any trusted key."));
                    }
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

struct OneCertHelper {
    cert: Cert,
}

impl VerificationHelper for OneCertHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(|r| r.is_ok()) {
                        return Ok(());
                    } else {
                        return Err(anyhow!("No valid signature found"));
                    }
                }
                _ => return Err(anyhow!("Unexpected message structure")),
            }
        }
        Err(anyhow!("No signature layer found"))
    }
}

pub fn cli_verify_signature(file_path: &str, sig_path: &str, key_name: &str) -> Result<()> {
    println!(
        "Verifying {} with signature {} using key '{}'",
        file_path, sig_path, key_name
    );

    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{}.asc", key_name));
    if !key_path.exists() {
        return Err(anyhow!("Key '{}' not found in local store.", key_name));
    }
    let key_bytes = fs::read(key_path)?;
    let cert = Cert::from_bytes(&key_bytes)?;

    verify_detached_signature(Path::new(file_path), Path::new(sig_path), &cert)?;

    println!("{}", "Signature is valid.".green());
    Ok(())
}

pub fn verify_detached_signature(
    data_path: &Path,
    signature_path: &Path,
    cert: &Cert,
) -> Result<()> {
    let policy = &StandardPolicy::new();
    let data = fs::read(data_path)?;
    let signature = fs::read(signature_path)?;

    let helper = OneCertHelper { cert: cert.clone() };

    let mut verifier =
        DetachedVerifierBuilder::from_bytes(&signature)?.with_policy(policy, None, helper)?;

    verifier.verify_bytes(&data)?;

    Ok(())
}

pub fn sign_detached(data_path: &Path, signature_path: &Path, key_id: &str) -> Result<()> {
    if !crate::utils::command_exists("gpg") {
        return Err(anyhow!(
            "gpg command not found. Please install GnuPG and ensure it's in your PATH."
        ));
    }

    let data_path_str = data_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid data path for signing."))?;
    let signature_path_str = signature_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid signature path for signing."))?;

    let output = Command::new("gpg")
        .arg("--batch")
        .arg("--yes")
        .arg("--detach-sign")
        .arg("--local-user")
        .arg(key_id)
        .arg("--output")
        .arg(signature_path_str)
        .arg(data_path_str)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut error_message = format!("gpg signing failed with status: {}.\n", output.status);

        if stderr.contains("No secret key") {
            error_message.push_str(&format!(
                "The secret key for '{}' was not found in your GPG keychain.\n",
                key_id
            ));
            error_message.push_str("Please ensure the key is imported into GPG and is trusted.");
        } else if stderr.contains("bad passphrase") || stderr.contains("Passphrase check failed") {
            error_message.push_str(
                "Incorrect passphrase provided, or the agent could not get the passphrase.\n",
            );
            error_message.push_str("Ensure your GPG agent is running and configured correctly if the key is password-protected.");
        } else {
            error_message.push_str(&format!("Stderr: {}", stderr));
        }

        return Err(anyhow!(error_message));
    }

    Ok(())
}

pub fn get_certs_by_name_or_fingerprint(identifiers: &[String]) -> Result<Vec<Cert>> {
    let all_keys = get_all_local_keys_info()?;
    let mut found_certs = Vec::new();

    for identifier in identifiers {
        let identifier_lower = identifier.to_lowercase();
        let mut found = false;
        for key_info in &all_keys {
            let fingerprint_lower = key_info.cert.fingerprint().to_string().to_lowercase();
            if key_info.name == *identifier || fingerprint_lower.starts_with(&identifier_lower) {
                found_certs.push(key_info.cert.clone());
                found = true;
                break;
            }
        }
        if !found {
            return Err(anyhow!(
                "Trusted key '{}' not found in Zoi's PGP keyring.",
                identifier
            ));
        }
    }
    Ok(found_certs)
}

pub fn verify_detached_signature_multi_key(
    data_path: &Path,
    signature_path: &Path,
    trusted_certs: Vec<Cert>,
) -> Result<()> {
    let policy = &StandardPolicy::new();
    let data = fs::read(data_path)?;
    let signature = fs::read(signature_path)?;

    let helper = MultiCertHelper {
        certs: trusted_certs,
    };

    let mut verifier =
        DetachedVerifierBuilder::from_bytes(&signature)?.with_policy(policy, None, helper)?;

    verifier.verify_bytes(&data)?;

    Ok(())
}
