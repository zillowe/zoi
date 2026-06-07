use std::io::Read;

use crate::utils;
use mlua::{self, Lua, Value};
use std::path::{Path, PathBuf};

use colored::Colorize;
use md5;
use sequoia_openpgp::{Cert, parse::Parse};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
pub fn add_verify_hash(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let verify_hash_fn = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut args_iter = args.into_iter();
        let file_path = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "verifyHash: first argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };
        let hash_str = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "verifyHash: second argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };

        let parts: Vec<&str> = hash_str.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(mlua::Error::RuntimeError(
                "Invalid hash format. Expected 'algo-hash'".to_string(),
            ));
        }
        let algo = parts[0];
        let expected_hash = parts[1];

        let p = Path::new(&file_path);
        let actual_path = if p.exists() {
            p.to_path_buf()
        } else if let Ok(build_dir) = lua.globals().get::<String>("BUILD_DIR") {
            Path::new(&build_dir).join(p)
        } else {
            p.to_path_buf()
        };

        let mut file = fs::File::open(&actual_path).map_err(|e| {
            mlua::Error::RuntimeError(format!("Failed to open file {:?}: {}", actual_path, e))
        })?;

        let actual_hash = match algo {
            "md5" => {
                let mut hasher = md5::Context::new();
                std::io::copy(&mut file, &mut hasher).map_err(|e| {
                    mlua::Error::RuntimeError(format!("Failed to read file: {}", e))
                })?;
                format!("{:x}", hasher.finalize())
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                let mut buffer = [0; 8192];
                loop {
                    let bytes_read = file.read(&mut buffer).map_err(|e| {
                        mlua::Error::RuntimeError(format!("Failed to read file: {}", e))
                    })?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                }
                hex::encode(hasher.finalize())
            }
            "sha512" => {
                let mut hasher = Sha512::new();
                let mut buffer = [0; 8192];
                loop {
                    let bytes_read = file.read(&mut buffer).map_err(|e| {
                        mlua::Error::RuntimeError(format!("Failed to read file: {}", e))
                    })?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                }
                hex::encode(hasher.finalize())
            }
            _ => {
                return Err(mlua::Error::RuntimeError(format!(
                    "Unsupported hash algorithm: {}",
                    algo
                )));
            }
        };

        if actual_hash.eq_ignore_ascii_case(expected_hash) {
            Ok(true)
        } else {
            if !quiet {
                println!(
                    "\n{}: Hash mismatch for {}",
                    "Error".red().bold(),
                    file_path.cyan()
                );
                println!("  specified: {}-{}", algo, expected_hash.yellow());
                println!("       got:    {}-{}", algo, actual_hash.green());
            }
            Ok(false)
        }
    })?;
    lua.globals().set("verifyHash", verify_hash_fn)?;
    Ok(())
}

pub fn add_verify_signature(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let verify_sig_fn = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut args_iter = args.into_iter();
        let file_path = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "verifySignature: first argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };
        let sig_path = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "verifySignature: second argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };
        let key_source = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "verifySignature: third argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };

        let resolve_path = |p_str: &str| -> PathBuf {
            let p = Path::new(p_str);
            if p.exists() {
                p.to_path_buf()
            } else if let Ok(build_dir) = lua.globals().get::<String>("BUILD_DIR") {
                Path::new(&build_dir).join(p)
            } else {
                p.to_path_buf()
            }
        };

        let key_bytes: Vec<u8> = if key_source.starts_with("http") {
            let client =
                utils::get_http_client().map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            match client.get(&key_source).send().and_then(|r| r.bytes()) {
                Ok(b) => b.to_vec(),
                Err(e) => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Failed to download key: {}",
                        e
                    )));
                }
            }
        } else {
            let resolved_key_path = resolve_path(&key_source);
            if resolved_key_path.exists() {
                match fs::read(&resolved_key_path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to read key file {:?}: {}",
                            resolved_key_path, e
                        )));
                    }
                }
            } else {
                let pgp_dir = match crate::pkg::pgp::get_pgp_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to get PGP dir: {}",
                            e
                        )));
                    }
                };
                let key_path = pgp_dir.join(format!("{}.asc", key_source));
                if !key_path.exists() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Key with name '{}' not found (checked locally and at {:?}).",
                        key_source, resolved_key_path
                    )));
                }
                match fs::read(&key_path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to read key file {:?}: {}",
                            key_path, e
                        )));
                    }
                }
            }
        };

        let cert = match Cert::from_bytes(&key_bytes) {
            Ok(c) => c,
            Err(e) => return Err(mlua::Error::RuntimeError(format!("Invalid PGP key: {}", e))),
        };

        let final_file_path = resolve_path(&file_path);
        let final_sig_path = resolve_path(&sig_path);

        let result =
            crate::pkg::pgp::verify_detached_signature(&final_file_path, &final_sig_path, &cert);

        match result {
            Ok(_) => Ok(true),
            Err(e) => {
                if !quiet {
                    eprintln!("Signature verification failed: {}", e);
                }
                Ok(false)
            }
        }
    })?;
    lua.globals().set("verifySignature", verify_sig_fn)?;
    Ok(())
}

pub fn add_add_pgp_key(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let add_pgp_key_fn = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut args_iter = args.into_iter();
        let source = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "addPgpKey: first argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };
        let name = match args_iter.next().unwrap_or(Value::Nil) {
            Value::String(s) => s.to_str()?.to_string(),
            v => {
                return Err(mlua::Error::RuntimeError(format!(
                    "addPgpKey: second argument must be a string, got {}",
                    v.type_name()
                )));
            }
        };

        let result = if source.starts_with("http") {
            crate::pkg::pgp::add_key_from_url(&source, &name, quiet)
        } else {
            let p = Path::new(&source);
            let actual_path = if p.exists() {
                p.to_path_buf()
            } else if let Ok(build_dir) = lua.globals().get::<String>("BUILD_DIR") {
                Path::new(&build_dir).join(p)
            } else {
                p.to_path_buf()
            };
            crate::pkg::pgp::add_key_from_path(
                actual_path.to_str().unwrap_or(&source),
                Some(&name),
                quiet,
            )
        };

        if let Err(e) = result {
            if !quiet {
                eprintln!("Failed to add PGP key '{}': {}", name, e);
            }
            return Ok(false);
        }
        Ok(true)
    })?;
    lua.globals().set("addPgpKey", add_pgp_key_fn)?;
    Ok(())
}
