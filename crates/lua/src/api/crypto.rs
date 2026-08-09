//! Cryptographic utilities for the Lua environment.
//!
//! This module provides functions to verify file integrity using hashes and
//! digital signatures, ensuring the security of downloaded assets.

use std::fs;
use std::path::{Path, PathBuf};

use colored::Colorize;
use mlua::{self, Lua, Value};
use sequoia_openpgp::Cert;
use sequoia_openpgp::parse::Parse;
use zoi_core::utils;
/// Exposes cryptographic utilities to the Lua environment.
///
/// These functions allow package scripts to perform security validations:
/// - `verifyHash`: Checks a file's SHA-256 or SHA-512 integrity.
/// - `verifySignature`: Validates a detached PGP signature using a local or
///   remote key.
/// - `addPgpKey`: Dynamically imports a trusted PGP key into the Zoi keyring.
///
/// This provides the "Chain of Trust" within the package build process,
/// ensuring that downloaded assets have not been tampered with.
///
/// # Errors
///
/// Returns an error if the cryptographic utilities cannot be added to the Lua
/// environment.
pub fn add_verify_hash(lua: &Lua, quiet: bool,) -> Result<(), mlua::Error,> {
    let verify_hash_fn =
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let mut args_iter = args.into_iter();
            let file_path = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "verifyHash: first argument must be a string, got {}",
                        v.type_name()
                    ),),);
                }
            };
            let hash_str = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "verifyHash: second argument must be a string, got {}",
                        v.type_name()
                    ),),);
                }
            };

            let parts: Vec<&str,> = hash_str.splitn(2, '-',).collect();
            if parts.len() != 2 {
                return Err(mlua::Error::RuntimeError(
                    "Invalid hash format. Expected 'algo-hash'".to_string(),
                ),);
            }
            let algo = parts.first().copied().unwrap_or_default();
            let expected_hash = parts.get(1,).copied().unwrap_or_default();

            let p = Path::new(&file_path,);
            let actual_path = if p.exists() {
                p.to_path_buf()
            } else if let Ok(build_dir,) =
                lua.globals().get::<String>("BUILD_DIR",)
            {
                Path::new(&build_dir,).join(p,)
            } else {
                p.to_path_buf()
            };

            let Some(hash_algo,) =
                zoi_core::hash::HashAlgorithm::from_name(algo,)
            else {
                return Err(mlua::Error::RuntimeError(format!(
                    "Unsupported hash algorithm: {algo}"
                ),),);
            };

            let actual_hash = match zoi_core::hash::calculate_file_hash(
                &actual_path,
                hash_algo,
            ) {
                Ok(h,) => h,
                Err(e,) => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Failed to calculate hash: {e}"
                    ),),);
                }
            };

            if actual_hash.eq_ignore_ascii_case(expected_hash,) {
                Ok(true,)
            } else {
                if !quiet {
                    println!(
                        "\n{}: Hash mismatch for {}",
                        "Error".red().bold(),
                        file_path.cyan()
                    );
                    println!("  specified: {algo}-{expected_hash}");
                    println!("       got:    {algo}-{actual_hash}");
                }
                Ok(false,)
            }
        },)?;
    lua.globals().set("verifyHash", verify_hash_fn,)?;
    Ok((),)
}

/// Adds the `verifySignature` function to the Lua global environment.
///
/// This function allows package scripts to validate detached PGP signatures.
///
/// # Errors
///
/// Returns an error if the `verifySignature` function cannot be added to the
/// Lua environment.
pub fn add_verify_signature(
    lua: &Lua,
    quiet: bool,
) -> Result<(), mlua::Error,> {
    let verify_sig_fn =
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let mut args_iter = args.into_iter();
            let file_path = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "verifySignature: first argument must be a string, \
                         got {}",
                        v.type_name()
                    ),),);
                }
            };
            let sig_path = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "verifySignature: second argument must be a string, \
                         got {}",
                        v.type_name()
                    ),),);
                }
            };
            let key_source = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "verifySignature: third argument must be a string, \
                         got {}",
                        v.type_name()
                    ),),);
                }
            };

            let resolve_path = |p_str: &str| -> PathBuf {
                let p = Path::new(p_str,);
                if p.exists() {
                    p.to_path_buf()
                } else if let Ok(build_dir,) =
                    lua.globals().get::<String>("BUILD_DIR",)
                {
                    Path::new(&build_dir,).join(p,)
                } else {
                    p.to_path_buf()
                }
            };

            let key_bytes: Vec<u8,> = if key_source.starts_with("http",) {
                let client = utils::get_http_client()
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string(),),)?;
                #[allow(clippy::redundant_closure_for_method_calls)]
                match client.get(&key_source,).send().and_then(|r| r.bytes(),) {
                    Ok(b,) => b.to_vec(),
                    Err(e,) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to download key: {e}"
                        ),),);
                    }
                }
            } else {
                let resolved_key_path = resolve_path(&key_source,);
                if resolved_key_path.exists() {
                    match fs::read(&resolved_key_path,) {
                        Ok(b,) => b,
                        Err(e,) => {
                            return Err(mlua::Error::RuntimeError(format!(
                                "Failed to read key file {}: {e}",
                                resolved_key_path.display()
                            ),),);
                        }
                    }
                } else {
                    let pgp_dir = match zoi_core::pgp::get_pgp_dir() {
                        Ok(dir,) => dir,
                        Err(e,) => {
                            return Err(mlua::Error::RuntimeError(format!(
                                "Failed to get PGP dir: {e}"
                            ),),);
                        }
                    };
                    let key_path = pgp_dir.join(format!("{key_source}.asc"),);
                    if !key_path.exists() {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Key with name '{key_source}' not found (checked \
                             locally and at {}).",
                            resolved_key_path.display()
                        ),),);
                    }
                    match fs::read(&key_path,) {
                        Ok(b,) => b,
                        Err(e,) => {
                            return Err(mlua::Error::RuntimeError(format!(
                                "Failed to read key file {}: {e}",
                                key_path.display()
                            ),),);
                        }
                    }
                }
            };

            let cert = match Cert::from_bytes(&key_bytes,) {
                Ok(c,) => c,
                Err(e,) => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Invalid PGP key: {e}"
                    ),),);
                }
            };

            let final_file_path = resolve_path(&file_path,);
            let final_sig_path = resolve_path(&sig_path,);

            let result = zoi_core::pgp::verify_detached_signature(
                &final_file_path,
                &final_sig_path,
                &cert,
            );

            match result {
                Ok((),) => Ok(true,),
                Err(e,) => {
                    if !quiet {
                        eprintln!("Signature verification failed: {e}");
                    }
                    Ok(false,)
                }
            }
        },)?;
    lua.globals().set("verifySignature", verify_sig_fn,)?;
    Ok((),)
}

/// Adds the `addPgpKey` function to the Lua global environment.
///
/// This function allows package scripts to import trusted PGP keys.
///
/// # Errors
///
/// Returns an error if the `addPgpKey` function cannot be added to the Lua
/// environment.
pub fn add_add_pgp_key(lua: &Lua, quiet: bool,) -> Result<(), mlua::Error,> {
    let add_pgp_key_fn =
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let mut args_iter = args.into_iter();
            let source = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "addPgpKey: first argument must be a string, got {}",
                        v.type_name()
                    ),),);
                }
            };
            let name = match args_iter.next().unwrap_or(Value::Nil,) {
                Value::String(s,) => s.to_str()?.to_string(),
                v => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "addPgpKey: second argument must be a string, got {}",
                        v.type_name()
                    ),),);
                }
            };

            let result = if source.starts_with("http",) {
                zoi_core::pgp::add_key_from_url(&source, &name, quiet,)
            } else {
                let p = Path::new(&source,);
                let actual_path = if p.exists() {
                    p.to_path_buf()
                } else if let Ok(build_dir,) =
                    lua.globals().get::<String>("BUILD_DIR",)
                {
                    Path::new(&build_dir,).join(p,)
                } else {
                    p.to_path_buf()
                };
                zoi_core::pgp::add_key_from_path(
                    actual_path.to_str().unwrap_or(&source,),
                    Some(&name,),
                    quiet,
                )
            };

            if let Err(e,) = result {
                if !quiet {
                    eprintln!("Failed to add PGP key '{name}': {e}");
                }
                return Ok(false,);
            }
            Ok(true,)
        },)?;
    lua.globals().set("addPgpKey", add_pgp_key_fn,)?;
    Ok((),)
}
