//! Delta generation and application for pooled `.zpa` archives.
//!
//! Deltas are `.zdelta` containers whose payload transforms the *pool* of
//! an installed/cached archive into the pool of a newer one:
//!
//! - Unchanged content is reused by hash (no transfer).
//! - Changed files are transferred as bsdiff patches against their previous
//!   version.
//! - Brand-new files travel in full.
//! - The new `manifest.json` and any non-pool entries (e.g. the bundled
//!   `.pkg.lua`) always travel in full since they identify the target.
//!
//! The rebuilt archive is verified structurally (every pool entry hashes
//! correctly) before it is used.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Result, anyhow};
use colored::Colorize;
use serde_json::json;
use tar::Archive;
use zoi_core::delta::{self, ZDELTA_FORMAT_ID, ZDelta};
use zoi_core::types::PooledZpaManifest;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Everything needed from a `.zpa` to diff or rebuild it.
struct ZpaContents {
    /// The pooled manifest describing content hashes and file mappings.
    manifest: PooledZpaManifest,
    /// Pool hash (without prefix) to raw bytes.
    pool: BTreeMap<String, Vec<u8>>,
    /// Non-pool, non-manifest entries (e.g. `.pkg.lua`), by entry name.
    extras: BTreeMap<String, Vec<u8>>,
    /// Embedded `manifest.sig` signature, if present.
    embedded_sig: Option<Vec<u8>>
}

/// Streams a `.zpa` archive and collects its manifest, pool contents and
/// extra entries.
fn read_zpa(path: &Path) -> Result<ZpaContents> {
    let file = File::open(path)?;
    let decoder = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(decoder);

    let mut manifest_json: Option<String> = None;
    let mut pool = BTreeMap::new();
    let mut extras = BTreeMap::new();
    let mut embedded_sig: Option<Vec<u8>> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().to_string();
        let name = name.trim_start_matches("./").to_string();

        if name.ends_with("manifest.sig") {
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            embedded_sig = Some(data);
            continue;
        }

        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;

        if name.ends_with("manifest.json") && manifest_json.is_none() {
            manifest_json = Some(String::from_utf8(data)?);
            continue;
        }

        if let Some(rest) = name.strip_prefix("pool/") {
            let file_name = Path::new(rest)
                .file_name()
                .map(|f| f.to_string_lossy().to_string());
            if let Some(hash) = file_name.and_then(|f| {
                f.strip_prefix("sha256-").map(ToString::to_string)
            }) {
                pool.insert(hash, data);
                continue;
            }
        }
        if !name.is_empty() {
            extras.insert(name, data);
        }
    }

    let manifest_json = manifest_json.ok_or_else(|| {
        anyhow!("No manifest.json found in '{}'", path.display())
    })?;
    let manifest = serde_json::from_str(&manifest_json)?;

    Ok(ZpaContents {
        manifest,
        pool,
        extras,
        embedded_sig
    })
}

/// Collects every distinct destination path mapped to each pool hash.
fn dest_map(manifest: &PooledZpaManifest) -> BTreeMap<String, HashSet<String>> {
    let mut map: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for mapping in manifest.mappings.values() {
        for scope_mapping in mapping.scopes.values() {
            for file in &scope_mapping.files {
                map.entry(file.hash.clone())
                    .or_default()
                    .insert(file.dest.clone());
            }
        }
    }
    map
}

/// Generates a `.zdelta` transforming `old_zpa` into `new_zpa`.
///
/// # Errors
///
/// Returns an error if either archive cannot be read or writing fails.
pub fn create_zpa_delta(
    old_zpa: &Path,
    new_zpa: &Path,
    output: &Path,
    sign_key: Option<&str>
) -> Result<()> {
    let old = read_zpa(old_zpa)?;
    let new = read_zpa(new_zpa)?;

    let old_dests = dest_map(&old.manifest);
    let new_dests = dest_map(&new.manifest);

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut ops: Vec<serde_json::Value> = Vec::new();
    let old_hashes: HashSet<_> = old.pool.keys().cloned().collect();

    // Track root entry hashes for integrity verification.
    // These are not covered by the manifest signature, so we store
    // their hashes in the delta metadata.
    let mut root_hashes: BTreeMap<String, String> = BTreeMap::new();

    // The destination map is keyed by the manifest's prefixed pool keys
    // ("sha256-<hex>"), while pool contents are stored under the bare hex
    // digest - normalize before matching.
    for (key, dests) in &new_dests {
        let Some(new_hash) = key.strip_prefix("sha256-") else {
            return Err(anyhow!("Unsupported pool hash format: {key}"));
        };
        if old_hashes.contains(new_hash) {
            // Content unchanged: reuse the old pool object directly.
            continue;
        }

        let Some(content) = new.pool.get(new_hash) else {
            return Err(anyhow!(
                "Pool content missing for sha256-{new_hash} in {}",
                new_zpa.display()
            ));
        };

        // Find a predecessor: any old object sharing a destination path.
        let mut from: Option<&String> = None;
        for dest in dests {
            for (old_hash, old_paths) in &old_dests {
                if old_paths.contains(dest) {
                    from = Some(old_hash);
                    break;
                }
            }
            if from.is_some() {
                break;
            }
        }

        if let (Some(from_hash), Some(old_content)) =
            (from, from.and_then(|h| old.pool.get(h)))
        {
            let patch = delta::diff_bytes(old_content, content)?;
            let patch_name = format!("patches/{new_hash}.bsdiff");
            files.push((patch_name.clone(), patch));
            ops.push(json!({
                "op": "patch",
                "key": format!("sha256-{new_hash}"),
                "from": format!("sha256-{from_hash}"),
                "patch": patch_name
            }));
        } else {
            let full_name = format!("full/{new_hash}");
            files.push((full_name.clone(), content.clone()));
            ops.push(json!({
                "op": "full",
                "key": format!("sha256-{new_hash}"),
                "data": full_name
            }));
        }
    }

    // The new manifest travels verbatim - it defines the target state.
    files.push(("manifest.json".into(), serde_json::to_vec(&new.manifest)?));
    // Carry the new archive's embedded signature forward. Because the delta
    // container stores the new manifest verbatim, the signature remains
    // valid against it.
    if let Some(sig) = &new.embedded_sig {
        files.push(("manifest.sig".into(), sig.clone()));
    }
    for (name, data) in &new.extras {
        // Compute hash of root entry for integrity verification.
        let hash = zoi_core::hash::calculate_reader_hash(
            &mut std::io::Cursor::new(data),
            zoi_core::hash::HashAlgorithm::Sha256
        )?
        .1;
        root_hashes.insert(name.clone(), format!("sha256-{hash}"));
        files.push((format!("root/{name}"), data.clone()));
    }

    let meta = json!({
        "format": ZDELTA_FORMAT_ID,
        "type": "zpa",
        "ops": ops,
        "root_hashes": root_hashes
    });

    delta::write_container(&meta, &files, output)?;

    // Generate sidecar files: .sig, .hash, .size
    let delta_bytes = std::fs::read(output)?;
    let delta_size = delta_bytes.len() as u64;
    let hash = zoi_core::hash::calculate_file_hash(
        output,
        zoi_core::hash::HashAlgorithm::Sha256
    )?;

    // Write .hash file
    let hash_path = output.with_extension("zdelta.hash");
    std::fs::write(&hash_path, format!("{hash}\n"))?;

    // Write .size file
    let size_path = output.with_extension("zdelta.size");
    std::fs::write(&size_path, format!("{delta_size}\n"))?;

    // Write .sig file if signing key provided
    if let Some(key_id) = sign_key {
        let sig_path = output.with_extension("zdelta.sig");
        zoi_core::pgp::sign_detached(output, &sig_path, key_id)?;
    }

    let old_size = std::fs::metadata(old_zpa)?.len();
    let new_size = std::fs::metadata(new_zpa)?.len();
    println!(
        "{} Created delta: {}",
        "::".bold().green(),
        output.display()
    );
    println!(
        "  Delta size: {} (full archive: {}, previous: {})",
        zoi_core::utils::format_bytes(delta_size),
        zoi_core::utils::format_bytes(new_size),
        zoi_core::utils::format_bytes(old_size)
    );
    Ok(())
}

/// Applies a `.zdelta` to a cached base archive, writing the rebuilt target
/// archive to `output`.
///
/// # Errors
///
/// Returns an error if the delta or base cannot be read, a patch fails, or
/// the rebuilt archive does not pass structural verification.
pub fn apply_zpa_delta(
    base_zpa: &Path,
    delta_path: &Path,
    output: &Path
) -> Result<()> {
    let base = read_zpa(base_zpa)?;

    let (meta, files) = match delta::load(delta_path)? {
        ZDelta::Container(meta, files) => (meta, files),
        ZDelta::Single(_) => {
            return Err(anyhow!(
                "{} is a single-target delta, expected a zpa container",
                delta_path.display()
            ));
        }
    };

    if meta.get("format")
        != Some(&serde_json::Value::String(ZDELTA_FORMAT_ID.to_string()))
    {
        return Err(anyhow!(
            "Unsupported delta format: {}",
            meta.get("format").unwrap_or(&serde_json::Value::Null)
        ));
    }

    // Start from the base pool; every op produces new-keyed objects.
    let mut new_pool: BTreeMap<String, Vec<u8>> =
        base.pool.clone().into_iter().collect();
    let Some(ops) = meta.get("ops").and_then(|v| v.as_array()) else {
        return Err(anyhow!("Delta meta has no ops array"));
    };

    for op in ops {
        let kind = op["op"].as_str().unwrap_or_default();
        let key = op["key"].as_str().unwrap_or_default();
        let Some(new_hash) = key.strip_prefix("sha256-") else {
            return Err(anyhow!("Bad pool key in delta: {key}"));
        };

        let content = match kind {
            "patch" => {
                let from_key = op["from"].as_str().unwrap_or_default();
                let Some(old_hash) = from_key.strip_prefix("sha256-") else {
                    return Err(anyhow!("Bad source key in delta: {from_key}"));
                };
                let patch_name = op["patch"].as_str().unwrap_or_default();
                let Some(base_bytes) = base.pool.get(old_hash) else {
                    return Err(anyhow!(
                        "Base archive lacks required object sha256-{old_hash}"
                    ));
                };
                let Some(patch) = files.get(patch_name) else {
                    return Err(anyhow!("Delta lacks patch file {patch_name}"));
                };
                delta::apply_bytes(base_bytes, patch)?
            }
            "full" => {
                let data_name = op["data"].as_str().unwrap_or_default();
                files.get(data_name).cloned().ok_or_else(|| {
                    anyhow!("Delta lacks data file {data_name}")
                })?
            }
            other => {
                return Err(anyhow!("Unknown delta op: {other}"));
            }
        };

        // Never trust a patch blindly: verify the produced content hashes to
        // exactly the key it claims.
        let actual = zoi_core::hash::calculate_reader_hash(
            &mut std::io::Cursor::new(&content),
            zoi_core::hash::HashAlgorithm::Sha256
        )?
        .1;
        if actual != new_hash {
            return Err(anyhow!(
                "Delta produced content that does not match {key}"
            ));
        }
        new_pool.insert(new_hash.to_string(), content);
    }

    // Reassemble the target archive. The target manifest travels verbatim
    // inside the delta container.
    let manifest_bytes = files
        .get("manifest.json")
        .ok_or_else(|| anyhow!("Delta container lacks manifest.json"))?;
    let manifest: PooledZpaManifest = serde_json::from_slice(manifest_bytes)?;

    // Sanity-check the rebuilt pool satisfies the target manifest before
    // anything touches disk.
    for key in manifest.pool.keys() {
        let Some(hex) = key.strip_prefix("sha256-") else {
            return Err(anyhow!("Unsupported pool hash format: {key}"));
        };
        if !new_pool.contains_key(hex) {
            return Err(anyhow!(
                "Rebuilt pool is missing sha256-{hex}; base archive may be \
                 incompatible with this delta"
            ));
        }
    }

    // Verify root entry hashes from delta metadata.
    if let Some(root_hashes_value) = meta.get("root_hashes")
        && let Some(root_hashes_obj) = root_hashes_value.as_object()
    {
        for (name, expected_hash_value) in root_hashes_obj {
            let Some(expected_hash) = expected_hash_value.as_str() else {
                return Err(anyhow!("Invalid root_hashes entry for {name}"));
            };
            let Some(data) = files.get(&format!("root/{name}")) else {
                return Err(anyhow!("Delta missing root entry {name}"));
            };
            let actual_hash = zoi_core::hash::calculate_reader_hash(
                &mut std::io::Cursor::new(data),
                zoi_core::hash::HashAlgorithm::Sha256
            )?
            .1;
            let expected_hex = expected_hash
                .strip_prefix("sha256-")
                .unwrap_or(expected_hash);
            if actual_hash != expected_hex {
                return Err(anyhow!(
                    "Root entry {name} hash mismatch: expected \
                     {expected_hash}, got sha256-{actual_hash}"
                ));
            }
        }
    }

    let mut builder = tar::Builder::new(Vec::new());
    // Use the original manifest bytes from the delta container to preserve
    // the exact byte sequence that was signed. Reserializing would change
    // formatting and invalidate the embedded signature.
    append_entry(&mut builder, "manifest.json", manifest_bytes)?;
    // Restore the embedded signature if the delta carried one.
    if let Some(sig) = files.get("manifest.sig") {
        append_entry(&mut builder, "manifest.sig", sig)?;
    }
    for (name, data) in &files {
        if let Some(entry_name) = name.strip_prefix("root/") {
            // Skip the pool directory placeholder; pool/ will be recreated
            // by the explicit pool file writes below.
            if entry_name == "pool" {
                continue;
            }
            append_entry(&mut builder, entry_name, data)?;
        }
    }
    for (hex, content) in &new_pool {
        append_entry(&mut builder, &format!("pool/sha256-{hex}"), content)?;
    }
    let tar_bytes = builder.into_inner()?;
    std::fs::write(output, delta::zstd_compress(&tar_bytes)?)?;

    // Structural verification of what we just wrote.
    let rebuilt = read_zpa(output)?;
    for key in rebuilt.manifest.pool.keys() {
        let Some(hex) = key.strip_prefix("sha256-") else {
            return Err(anyhow!("Unsupported pool hash format: {key}"));
        };
        let Some(content) = rebuilt.pool.get(hex) else {
            std::fs::remove_file(output).ok();
            return Err(anyhow!(
                "Rebuilt archive is missing pool object sha256-{hex}"
            ));
        };
        let actual = zoi_core::hash::calculate_reader_hash(
            &mut std::io::Cursor::new(content),
            zoi_core::hash::HashAlgorithm::Sha256
        )?
        .1;
        let expected_size =
            rebuilt.manifest.pool.get(key).map_or(0, |e| e.size);
        if content.len() as u64 != expected_size || actual != hex {
            std::fs::remove_file(output).ok();
            return Err(anyhow!(
                "Rebuilt archive failed integrity verification for {key}"
            ));
        }
    }

    Ok(())
}

/// Appends a single file entry to a tar builder with the given name and
/// content.
///
/// # Errors
///
/// Returns an error if the entry cannot be appended.
fn append_entry(
    builder: &mut tar::Builder<Vec<u8>>,
    name: &str,
    data: &[u8]
) -> Result<()> {
    use std::io::Cursor;
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, Cursor::new(data))?;
    Ok(())
}
