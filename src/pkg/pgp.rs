use colored::*;
use sequoia_openpgp::Cert;
use sequoia_openpgp::parse::Parse;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn get_pgp_dir() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Could not find home directory.")?;
    let pgp_dir = home_dir.join(".zoi").join("pgps");
    fs::create_dir_all(&pgp_dir)?;
    Ok(pgp_dir)
}

pub fn add_key_from_bytes(key_bytes: &[u8], name: &str) -> Result<(), Box<dyn Error>> {
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

pub fn add_key_from_path(path: &str, name: Option<&str>) -> Result<(), Box<dyn Error>> {
    let key_path = Path::new(path);
    if !key_path.exists() {
        return Err(format!("Key file not found at: {}", path).into());
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

pub fn add_key_from_fingerprint(fingerprint: &str, name: &str) -> Result<(), Box<dyn Error>> {
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
        return Err(format!(
            "Failed to fetch key from keyserver (HTTP {}).",
            response.status()
        )
        .into());
    }

    let key_bytes = response.bytes()?.to_vec();

    println!("Validating PGP key...");
    Cert::from_bytes(&key_bytes)?;
    println!("{}", "Key is valid.".green());

    add_key_from_bytes(&key_bytes, name)
}

pub fn remove_key_by_name(name: &str) -> Result<(), Box<dyn Error>> {
    let pgp_dir = get_pgp_dir()?;
    let key_path = pgp_dir.join(format!("{}.asc", name));

    if !key_path.exists() {
        return Err(format!("Key with name '{}' not found.", name).into());
    }

    fs::remove_file(&key_path)?;
    println!("Successfully removed key '{}'.", name.cyan());

    Ok(())
}

pub fn remove_key_by_fingerprint(fingerprint: &str) -> Result<(), Box<dyn Error>> {
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

    Err(format!("Key with fingerprint '{}' not found.", fingerprint).into())
}

pub fn get_all_local_certs() -> Result<Vec<Cert>, Box<dyn Error>> {
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
