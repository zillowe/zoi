use crate::utils;
use flate2::read::GzDecoder;
use md5;
use mlua::{self, Lua, LuaSerdeExt, Table, Value};
use sequoia_openpgp::{Cert, parse::Parse};
use serde::Deserialize;
use sha2::{Digest, Sha256, Sha512};
use std::io::Read;
use std::path::PathBuf;
use std::{fs, path::Path};
use urlencoding;
use walkdir::WalkDir;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

fn add_parse_util(lua: &Lua) -> Result<(), mlua::Error> {
    let parse_table = lua.create_table()?;

    let json_fn = lua.create_function(|lua, json_str: String| {
        let value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("json", json_fn)?;

    let yaml_fn = lua.create_function(|lua, yaml_str: String| {
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml_str)
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("yaml", yaml_fn)?;

    let toml_fn = lua.create_function(|lua, toml_str: String| {
        let value: toml::Value =
            toml::from_str(&toml_str).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        lua.to_value(&value)
    })?;
    parse_table.set("toml", toml_fn)?;

    let checksum_fn = lua.create_function(|_, (content, file_name): (String, String)| {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 && parts[1] == file_name {
                return Ok(Some(parts[0].to_string()));
            }
        }
        Ok(None)
    })?;
    parse_table.set("checksumFile", checksum_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("PARSE", parse_table)?;

    Ok(())
}

fn add_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let fetch_table = lua.create_table()?;

    let fetch_fn = lua.create_function(|_, url: String| -> Result<String, mlua::Error> {
        let response =
            reqwest::blocking::get(url).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        let text = response
            .text()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        Ok(text)
    })?;
    fetch_table.set("url", fetch_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FETCH", fetch_table)?;

    Ok(())
}

#[derive(Deserialize)]
struct GitArgs {
    repo: String,
    domain: Option<String>,
    branch: Option<String>,
}

fn fetch_json(url: &str) -> Result<serde_json::Value, mlua::Error> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("zoi")
        .build()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(mlua::Error::RuntimeError(format!(
            "Request to {} failed with status: {} and body: {}",
            url,
            response.status(),
            response.text().unwrap_or_else(|_| "N/A".to_string())
        )));
    }

    let text = response
        .text()
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| mlua::Error::RuntimeError(e.to_string()))
}

fn add_git_fetch_util(lua: &Lua) -> Result<(), mlua::Error> {
    let utils_table: Table = lua.globals().get("UTILS")?;
    let fetch_table: Table = utils_table.get("FETCH")?;

    for provider in ["GITHUB", "GITLAB", "GITEA", "FORGEJO"] {
        let provider_table = lua.create_table()?;
        let latest_table = lua.create_table()?;

        for what in ["tag", "release", "commit"] {
            let get_latest_fn = lua.create_function(move |lua, args: Table| {
                let git_args: GitArgs = lua
                    .from_value(Value::Table(args))
                    .map_err(|e| mlua::Error::RuntimeError(format!("Invalid arguments: {}", e)))?;

                let base_url = match provider {
                    "GITHUB" => git_args
                        .domain
                        .unwrap_or_else(|| "https://api.github.com".to_string()),
                    "GITLAB" => git_args
                        .domain
                        .unwrap_or_else(|| "https://gitlab.com".to_string()),
                    "GITEA" => git_args
                        .domain
                        .unwrap_or_else(|| "https://gitea.com".to_string()),
                    "FORGEJO" => git_args
                        .domain
                        .unwrap_or_else(|| "https://codeberg.org".to_string()),
                    _ => unreachable!(),
                };

                let url = match (provider, what) {
                    ("GITHUB", "tag") => format!("{}/repos/{}/tags", base_url, git_args.repo),
                    ("GITHUB", "release") => {
                        format!("{}/repos/{}/releases/latest", base_url, git_args.repo)
                    }
                    ("GITHUB", "commit") => format!(
                        "{}/repos/{}/commits?sha={}",
                        base_url,
                        git_args.repo,
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),

                    ("GITLAB", "tag") => format!(
                        "{}/api/v4/projects/{}/repository/tags",
                        base_url,
                        urlencoding::encode(&git_args.repo)
                    ),
                    ("GITLAB", "release") => format!(
                        "{}/api/v4/projects/{}/releases",
                        base_url,
                        urlencoding::encode(&git_args.repo)
                    ),
                    ("GITLAB", "commit") => format!(
                        "{}/api/v4/projects/{}/repository/commits?ref_name={}",
                        base_url,
                        urlencoding::encode(&git_args.repo),
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),

                    ("GITEA" | "FORGEJO", "tag") => {
                        format!("{}/api/v1/repos/{}/tags", base_url, git_args.repo)
                    }
                    ("GITEA" | "FORGEJO", "release") => {
                        format!(
                            "{}/api/v1/repos/{}/releases/latest",
                            base_url, git_args.repo
                        )
                    }
                    ("GITEA" | "FORGEJO", "commit") => format!(
                        "{}/api/v1/repos/{}/commits?sha={}",
                        base_url,
                        git_args.repo,
                        git_args.branch.as_deref().unwrap_or("HEAD")
                    ),
                    _ => unreachable!(),
                };

                let json = fetch_json(&url)?;

                let result = match (provider, what) {
                    ("GITHUB", "tag") | ("GITEA", "tag") | ("FORGEJO", "tag") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|t| t["name"].as_str()),
                    ("GITHUB", "release") | ("GITEA", "release") | ("FORGEJO", "release") => {
                        json["tag_name"].as_str()
                    }
                    ("GITHUB", "commit") | ("GITEA", "commit") | ("FORGEJO", "commit") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|c| c["sha"].as_str()),

                    ("GITLAB", "tag") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|t| t["name"].as_str()),
                    ("GITLAB", "release") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|r| r["tag_name"].as_str()),
                    ("GITLAB", "commit") => json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|c| c["id"].as_str()),
                    _ => unreachable!(),
                };

                result.map(|s| s.to_string()).ok_or_else(|| {
                    mlua::Error::RuntimeError(
                        "Could not extract value from API response".to_string(),
                    )
                })
            })?;
            latest_table.set(what, get_latest_fn)?;
        }

        provider_table.set("LATEST", latest_table)?;
        fetch_table.set(provider, provider_table)?;
    }

    Ok(())
}

fn add_file_util(lua: &Lua) -> Result<(), mlua::Error> {
    let file_fn = lua.create_function(
        |_, (url, path): (String, String)| -> Result<(), mlua::Error> {
            let response = reqwest::blocking::get(url)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let content = response
                .bytes()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            fs::write(path, content).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            Ok(())
        },
    )?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FILE", file_fn)?;

    Ok(())
}

fn add_import_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let import_fn = lua.create_function(move |lua, file_name: String| {
        let path = current_path_buf.parent().unwrap().join(&file_name);
        let content =
            fs::read_to_string(&path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            match extension {
                "json" => {
                    let value: serde_json::Value = serde_json::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                "yaml" | "yml" => {
                    let value: serde_yaml::Value = serde_yaml::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                "toml" => {
                    let value: toml::Value = toml::from_str(&content)
                        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                    return lua.to_value(&value);
                }
                _ => {
                    return lua.to_value(&content);
                }
            }
        }

        lua.to_value(&content)
    })?;
    lua.globals().set("IMPORT", import_fn)?;
    Ok(())
}

fn add_include_util(lua: &Lua, current_path: &Path) -> Result<(), mlua::Error> {
    let current_path_buf = current_path.to_path_buf();
    let include_fn =
        lua.create_function(move |lua, file_name: String| -> Result<(), mlua::Error> {
            let path = current_path_buf.parent().unwrap().join(file_name);
            let code =
                fs::read_to_string(path).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            lua.load(&code).exec()?;
            Ok(())
        })?;
    lua.globals().set("INCLUDE", include_fn)?;
    Ok(())
}

fn add_zcp(lua: &Lua) -> Result<(), mlua::Error> {
    let zcp_fn = lua.create_function(|lua, (source, destination): (String, String)| {
        let ops_table: Table = match lua.globals().get("__ZoiBuildOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals().set("__ZoiBuildOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zcp")?;
        op.set("source", source)?;
        op.set("destination", destination)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zcp", zcp_fn)?;
    Ok(())
}

fn add_verify_hash(lua: &Lua) -> Result<(), mlua::Error> {
    let verify_hash_fn = lua.create_function(|_lua, (file_path, hash_str): (String, String)| {
        let parts: Vec<&str> = hash_str.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(mlua::Error::RuntimeError(
                "Invalid hash format. Expected 'algo-hash'".to_string(),
            ));
        }
        let algo = parts[0];
        let expected_hash = parts[1];

        let mut file = fs::File::open(&file_path)
            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to open file: {}", e)))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to read file: {}", e)))?;

        let actual_hash = match algo {
            "md5" => {
                let digest = md5::compute(&buffer);
                format!("{:x}", digest)
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(&buffer);
                hex::encode(hasher.finalize())
            }
            "sha512" => {
                let mut hasher = Sha512::new();
                hasher.update(&buffer);
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
            println!(
                "Hash mismatch for {}: expected {}, got {}",
                file_path, expected_hash, actual_hash
            );
            Ok(false)
        }
    })?;
    lua.globals().set("verifyHash", verify_hash_fn)?;
    Ok(())
}

fn add_zrm(lua: &Lua) -> Result<(), mlua::Error> {
    let zrm_fn = lua.create_function(|lua, path: String| {
        let ops_table: Table = match lua.globals().get("__ZoiUninstallOperations") {
            Ok(t) => t,
            Err(_) => {
                let new_t = lua.create_table()?;
                lua.globals()
                    .set("__ZoiUninstallOperations", new_t.clone())?;
                new_t
            }
        };
        let op = lua.create_table()?;
        op.set("op", "zrm")?;
        op.set("path", path)?;
        ops_table.push(op)?;
        Ok(())
    })?;
    lua.globals().set("zrm", zrm_fn)?;
    Ok(())
}

fn add_cmd_util(lua: &Lua) -> Result<(), mlua::Error> {
    let cmd_fn = lua.create_function(|_, command: String| {
        println!("Executing: {}", command);
        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("pwsh")
                .arg("-Command")
                .arg(&command)
                .output()
        } else {
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
                .output()
        };

        match output {
            Ok(out) => {
                if !out.status.success() {
                    eprintln!("[cmd] {}", String::from_utf8_lossy(&out.stderr));
                }
                Ok(out.status.success())
            }
            Err(e) => {
                eprintln!("[cmd] Failed to execute command: {}", e);
                Ok(false)
            }
        }
    })?;
    lua.globals().set("cmd", cmd_fn)?;
    Ok(())
}

fn add_find_util(lua: &Lua) -> Result<(), mlua::Error> {
    let find_table = lua.create_table()?;

    let find_file_fn = lua.create_function(|lua, (dir, name): (String, String)| {
        let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
        let search_dir = Path::new(&build_dir_str).join(dir);
        for entry in WalkDir::new(search_dir) {
            let entry = entry.map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            if entry.file_name().to_string_lossy() == name {
                let path = entry.path();
                let relative_path = path.strip_prefix(Path::new(&build_dir_str)).unwrap();
                return Ok(Some(relative_path.to_string_lossy().to_string()));
            }
        }
        Ok(None)
    })?;
    find_table.set("file", find_file_fn)?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("FIND", find_table)?;

    Ok(())
}

fn add_extract_util(lua: &Lua) -> Result<(), mlua::Error> {
    let extract_fn = lua.create_function(|lua, (source, out_name): (String, Option<String>)| {
        let build_dir_str: String = lua.globals().get("BUILD_DIR")?;
        let build_dir = Path::new(&build_dir_str);

        let archive_file = if source.starts_with("http") {
            println!("Downloading: {}", source);
            let file_name = source.split('/').next_back().unwrap_or("download.tmp");
            let temp_path = build_dir.join(file_name);
            let response = reqwest::blocking::get(&source)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let content = response
                .bytes()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            fs::write(&temp_path, content).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            temp_path
        } else {
            PathBuf::from(source)
        };

        let out_dir_name = out_name.unwrap_or_else(|| "extracted".to_string());
        let out_dir = build_dir.join(out_dir_name);
        fs::create_dir_all(&out_dir).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        println!(
            "Extracting {} to {}",
            archive_file.display(),
            out_dir.display()
        );

        let file =
            fs::File::open(&archive_file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        let archive_path_str = archive_file.to_string_lossy();

        if archive_path_str.ends_with(".zip") {
            let mut archive =
                ZipArchive::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            archive
                .extract(&out_dir)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else if archive_path_str.ends_with(".tar.gz") {
            let tar_gz = GzDecoder::new(file);
            let mut archive = tar::Archive::new(tar_gz);
            archive
                .unpack(&out_dir)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else if archive_path_str.ends_with(".tar.zst") {
            let tar_zst =
                ZstdDecoder::new(file).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut archive = tar::Archive::new(tar_zst);
            archive
                .unpack(&out_dir)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else if archive_path_str.ends_with(".tar.xz") {
            let tar_xz = XzDecoder::new(file);
            let mut archive = tar::Archive::new(tar_xz);
            archive
                .unpack(&out_dir)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else {
            return Err(mlua::Error::RuntimeError(format!(
                "Unsupported archive format for file: {}",
                archive_path_str
            )));
        }

        Ok(())
    })?;

    let utils_table: Table = lua.globals().get("UTILS")?;
    utils_table.set("EXTRACT", extract_fn)?;

    Ok(())
}

fn add_verify_signature(lua: &Lua) -> Result<(), mlua::Error> {
    let verify_sig_fn = lua.create_function(
        |_, (file_path, sig_path, key_source): (String, String, String)| {
            let key_bytes: Vec<u8> = if key_source.starts_with("http") {
                match reqwest::blocking::get(&key_source).and_then(|r| r.bytes()) {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to download key: {}",
                            e
                        )));
                    }
                }
            } else if Path::new(&key_source).exists() {
                match fs::read(&key_source) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to read key file: {}",
                            e
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
                        "Key with name '{}' not found.",
                        key_source
                    )));
                }
                match fs::read(&key_path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Failed to read key file: {}",
                            e
                        )));
                    }
                }
            };

            let cert = match Cert::from_bytes(&key_bytes) {
                Ok(c) => c,
                Err(e) => return Err(mlua::Error::RuntimeError(format!("Invalid PGP key: {}", e))),
            };

            let result = crate::pkg::pgp::verify_detached_signature(
                Path::new(&file_path),
                Path::new(&sig_path),
                &cert,
            );

            match result {
                Ok(_) => Ok(true),
                Err(e) => {
                    eprintln!("Signature verification failed: {}", e);
                    Ok(false)
                }
            }
        },
    )?;
    lua.globals().set("verifySignature", verify_sig_fn)?;
    Ok(())
}

fn add_add_pgp_key(lua: &Lua) -> Result<(), mlua::Error> {
    let add_pgp_key_fn = lua.create_function(|_, (source, name): (String, String)| {
        let result = if source.starts_with("http") {
            crate::pkg::pgp::add_key_from_url(&source, &name)
        } else {
            crate::pkg::pgp::add_key_from_path(&source, Some(&name))
        };

        if let Err(e) = result {
            eprintln!("Failed to add PGP key '{}': {}", name, e);
            return Ok(false);
        }
        Ok(true)
    })?;
    lua.globals().set("addPgpKey", add_pgp_key_fn)?;
    Ok(())
}

pub fn add_package_lifecycle_functions(lua: &Lua) -> Result<(), mlua::Error> {
    let metadata_fn = lua.create_function(move |lua, pkg_def: Table| {
        if let Ok(meta_table) = lua.globals().get::<Table>("__ZoiPackageMeta")
            && let Ok(pkg_global) = lua.globals().get::<Table>("PKG")
        {
            for pair in pkg_def.pairs::<Value, Value>() {
                let (key, value) = pair?;
                meta_table.set(key.clone(), value.clone())?;
                pkg_global.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("metadata", metadata_fn)?;

    let dependencies_fn = lua.create_function(move |lua, deps_def: Table| {
        if let Ok(deps_table) = lua.globals().get::<Table>("__ZoiPackageDeps") {
            for pair in deps_def.pairs::<String, Value>() {
                let (key, value) = pair?;
                deps_table.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("dependencies", dependencies_fn)?;

    let updates_fn = lua.create_function(move |lua, updates_list: Table| {
        if let Ok(updates_table) = lua.globals().get::<Table>("__ZoiPackageUpdates") {
            for pair in updates_list.pairs::<Value, Table>() {
                let (_, update_info) = pair?;
                updates_table.push(update_info)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("updates", updates_fn)?;

    let hooks_fn = lua.create_function(move |lua, hooks_def: Table| {
        if let Ok(hooks_table) = lua.globals().get::<Table>("__ZoiPackageHooks") {
            for pair in hooks_def.pairs::<String, Value>() {
                let (key, value) = pair?;
                hooks_table.set(key, value)?;
            }
        }
        Ok(())
    })?;
    lua.globals().set("hooks", hooks_fn)?;

    let prepare_fn = lua.create_function(|_, _: Table| Ok(()))?;
    lua.globals().set("prepare", prepare_fn)?;
    let package_fn = lua.create_function(|_, _: Table| Ok(()))?;
    lua.globals().set("package", package_fn)?;
    let verify_fn = lua.create_function(|_, _: Table| Ok(()))?;
    lua.globals().set("verify", verify_fn)?;
    let uninstall_fn = lua.create_function(|_, _: Table| Ok(()))?;
    lua.globals().set("uninstall", uninstall_fn)?;

    Ok(())
}

pub fn setup_lua_environment(
    lua: &Lua,
    platform: &str,
    version_override: Option<&str>,
    file_path: Option<&str>,
    create_pkg_dir: Option<&str>,
) -> Result<(), mlua::Error> {
    let system_table = lua.create_table()?;
    let parts: Vec<&str> = platform.split('-').collect();
    system_table.set("OS", *parts.first().unwrap_or(&""))?;
    system_table.set("ARCH", *parts.get(1).unwrap_or(&""))?;
    if let Some(distro) = utils::get_linux_distribution() {
        system_table.set("DISTRO", distro)?;
    }
    if let Some(manager) = utils::get_native_package_manager() {
        system_table.set("MANAGER", manager)?;
    }
    lua.globals().set("SYSTEM", system_table)?;

    let zoi_table = lua.create_table()?;
    if let Some(ver) = version_override {
        zoi_table.set("VERSION", ver)?;
    }

    if let Some(dir) = create_pkg_dir {
        zoi_table.set("CREATE_PKG_DIR", dir)?;
    }

    let path_table = lua.create_table()?;
    if let Some(home_dir) = home::home_dir() {
        path_table.set("user", home_dir.join(".zoi").to_string_lossy().to_string())?;
    }

    let system_bin_path = if cfg!(target_os = "windows") {
        "C:\\ProgramData\\zoi\\pkgs\\bin".to_string()
    } else {
        "/usr/local/bin".to_string()
    };
    path_table.set("system", system_bin_path)?;

    zoi_table.set("PATH", path_table)?;
    lua.globals().set("ZOI", zoi_table)?;

    let utils_table = lua.create_table()?;
    lua.globals().set("UTILS", utils_table)?;

    add_fetch_util(lua)?;
    add_parse_util(lua)?;
    add_git_fetch_util(lua)?;
    add_file_util(lua)?;
    add_zcp(lua)?;
    add_verify_hash(lua)?;
    add_zrm(lua)?;
    add_cmd_util(lua)?;
    add_find_util(lua)?;
    add_extract_util(lua)?;
    add_verify_signature(lua)?;
    add_add_pgp_key(lua)?;
    add_package_lifecycle_functions(lua)?;

    if let Some(path_str) = file_path {
        let path = Path::new(path_str);
        add_import_util(lua, path)?;
        add_include_util(lua, path)?;
    }

    Ok(())
}
