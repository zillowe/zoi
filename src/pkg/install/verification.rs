use crate::pkg::types;

use crate::utils;
use anyhow::Result;
use colored::*;
use sequoia_openpgp::{
    KeyHandle,
    cert::Cert,
    parse::{
        Parse,
        stream::{DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper},
    },
    policy::StandardPolicy,
};
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use tokio::runtime::Runtime;

fn get_expected_checksum(
    checksums: &types::Checksums,
    file_to_verify: &str,
    _pkg: &types::Package,
    _platform: &str,
) -> Result<Option<(String, String)>, Box<dyn Error>> {
    match checksums {
        types::Checksums::Url(url) => {
            println!("Downloading checksums from: {}", url.cyan());
            let response = reqwest::blocking::get(url)?.text()?;
            for line in response.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 && parts[1] == file_to_verify {
                    return Ok(Some((parts[0].to_string(), "sha512".to_string())));
                }
            }
            if response.lines().count() == 1 && response.split_whitespace().count() == 1 {
                return Ok(Some((response.trim().to_string(), "sha512".to_string())));
            }
            Ok(None)
        }
        types::Checksums::List {
            checksum_type,
            items,
        } => {
            for item in items {
                if item.file == file_to_verify {
                    if item.checksum.starts_with("http") {
                        println!("Downloading checksum from: {}", item.checksum.cyan());
                        let response = reqwest::blocking::get(&item.checksum)?.text()?;
                        return Ok(Some((response.trim().to_string(), checksum_type.clone())));
                    } else {
                        return Ok(Some((item.checksum.clone(), checksum_type.clone())));
                    }
                }
            }
            Ok(None)
        }
    }
}

pub fn verify_checksum(
    data: &[u8],
    method: &types::InstallationMethod,
    pkg: &types::Package,
    file_to_verify: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(checksums) = &method.checksums {
        println!("Verifying checksum for {}...", file_to_verify);
        let platform = utils::get_platform()?;
        if let Some((expected_checksum, checksum_type)) =
            get_expected_checksum(checksums, file_to_verify, pkg, &platform)?
        {
            let computed_checksum = match checksum_type.as_str() {
                "sha256" => {
                    let mut hasher = Sha256::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
                _ => {
                    let mut hasher = Sha512::new();
                    hasher.update(data);
                    format!("{:x}", hasher.finalize())
                }
            };

            if computed_checksum.eq_ignore_ascii_case(&expected_checksum) {
                println!("{}", "Checksum verified successfully.".green());
                Ok(())
            } else {
                Err(format!(
                    "Checksum mismatch for {}.\nExpected: {}\nComputed: {}",
                    file_to_verify, expected_checksum, computed_checksum
                )
                .into())
            }
        } else {
            println!(
                "{} No checksum found for file '{}'. Skipping verification.",
                "Warning:".yellow(),
                file_to_verify
            );
            Ok(())
        }
    } else {
        Ok(())
    }
}

struct Helper {
    certs: Vec<Cert>,
}

impl VerificationHelper for Helper {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> anyhow::Result<Vec<Cert>> {
        let matching_certs: Vec<Cert> = self
            .certs
            .iter()
            .filter(|cert| {
                ids.iter().any(|id| {
                    cert.keys().any(|key| match *id {
                        KeyHandle::KeyID(ref keyid) => key.key().keyid() == *keyid,
                        KeyHandle::Fingerprint(ref fp) => key.key().fingerprint() == *fp,
                    })
                })
            })
            .cloned()
            .collect();
        Ok(matching_certs)
    }

    fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
        if let Some(layer) = structure.into_iter().next() {
            match layer {
                MessageLayer::SignatureGroup { results } => {
                    if results.iter().any(|r| r.is_ok()) {
                        return Ok(());
                    } else {
                        return Err(anyhow::anyhow!("No valid signature found"));
                    }
                }
                _ => return Err(anyhow::anyhow!("Unexpected message structure")),
            }
        }
        Err(anyhow::anyhow!("No signature layer found"))
    }
}

pub fn verify_prebuilt_signature(
    data: &[u8],
    signature_bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    println!("Verifying signature of pre-built package...");

    let certs = crate::pkg::pgp::get_all_local_certs()?;
    if certs.is_empty() {
        return Err("No PGP keys found in local store to verify signature. Please run 'zoi sync' or add keys manually.".into());
    }

    let policy = &StandardPolicy::new();
    let helper = Helper { certs };

    let mut verifier =
        DetachedVerifierBuilder::from_bytes(signature_bytes)?.with_policy(policy, None, helper)?;

    verifier.verify_bytes(data)?;

    println!("{}", "Signature verified successfully.".green());
    Ok(())
}

pub fn verify_signatures(
    data: &[u8],
    method: &types::InstallationMethod,
    pkg: &types::Package,
    file_to_verify: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(sigs) = &method.sigs {
        let sig_info = sigs.iter().find(|s| s.file == file_to_verify);

        if let Some(sig_info) = sig_info {
            println!("Verifying signature for {}...", file_to_verify);

            struct KeyInfo<'a> {
                key_source: &'a str,
                key_name: Option<&'a str>,
                one_time: bool,
            }

            let mut keys_to_process: Vec<KeyInfo> = Vec::new();
            if let Some(key) = pkg.maintainer.key.as_deref() {
                keys_to_process.push(KeyInfo {
                    key_source: key,
                    key_name: pkg.maintainer.key_name.as_deref(),
                    one_time: pkg.maintainer.one_time,
                });
            }
            if let Some(author) = &pkg.author
                && let Some(key) = author.key.as_deref()
            {
                keys_to_process.push(KeyInfo {
                    key_source: key,
                    key_name: author.key_name.as_deref(),
                    one_time: author.one_time,
                });
            }

            if keys_to_process.is_empty() {
                println!(
                    "{} Signature found for '{}', but no maintainer or author key is defined. Skipping verification.",
                    "Warning:".yellow(),
                    file_to_verify
                );
                return Ok(());
            }

            let rt = Runtime::new()?;
            rt.block_on(async {
                let mut certs = Vec::new();
                for key_info in &keys_to_process {
                    let key_bytes_result = if key_info.key_source.starts_with("http") {
                        println!("Importing key from URL: {}", key_info.key_source.cyan());
                        reqwest::get(key_info.key_source).await?.bytes().await
                    } else if key_info.key_source.len() == 40
                        && key_info.key_source.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        let fingerprint = key_info.key_source.to_uppercase();
                        let key_server_url = format!(
                            "https://keys.openpgp.org/vks/v1/by-fingerprint/{}",
                            fingerprint
                        );
                        println!(
                            "Importing key for fingerprint {} from keyserver...",
                            fingerprint.cyan()
                        );
                        reqwest::get(&key_server_url).await?.bytes().await
                    } else {
                        println!(
                            "{} Invalid key source: '{}'. Must be a URL or a 40-character GPG fingerprint.",
                            "Warning:".yellow(),
                            key_info.key_source
                        );
                        continue;
                    };

                    match key_bytes_result {
                        Ok(key_bytes) => {
                            if let Ok(cert) = Cert::from_bytes(&key_bytes) {
                                if !key_info.one_time
                                    && let Some(name) = key_info.key_name
                                        && let Err(e) =
                                            crate::pkg::pgp::add_key_from_bytes(&key_bytes, name)
                                        {
                                            println!(
                                                "{} Failed to save key for {}: {}",
                                                "Warning:".yellow(),
                                                name,
                                                e
                                            );
                                        }
                                certs.push(cert);
                            } else {
                                println!(
                                    "{} Failed to parse certificate from source: {}",
                                    "Warning:".yellow(),
                                    key_info.key_source
                                );
                            }
                        }
                        Err(e) => {
                            println!(
                                "{} Failed to download key from source {}: {}",
                                "Warning:".yellow(),
                                key_info.key_source,
                                e
                            );
                        }
                    }
                }

                if certs.is_empty() {
                    return Err(anyhow::anyhow!(
                        "No valid public keys found to verify signature."
                    ));
                }

                println!("Downloading signature from: {}", sig_info.sig);
                let sig_bytes = reqwest::get(&sig_info.sig).await?.bytes().await?;

                let policy = &StandardPolicy::new();
                let helper = Helper { certs };

                let mut verifier = DetachedVerifierBuilder::from_bytes(&sig_bytes)?
                    .with_policy(policy, None, helper)?;

                verifier.verify_bytes(data)?;

                println!("{}", "Signature verified successfully.".green());
                Ok(())
            })?;
        }
    }
    Ok(())
}
