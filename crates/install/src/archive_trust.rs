//! Shared rules for the trust-relevant entries of package archives.
//!
//! The signature verifier and the extractor must agree on which bytes are
//! "the manifest" and "the package definition". If they each picked their own
//! copy, an archive carrying duplicate or nested entries could show one file
//! to the verifier and hand another to the installer. Both therefore classify
//! entries through this module and reject anything ambiguous:
//!
//! - `manifest.json` / `manifest.sig` are only accepted once each, as regular
//!   files at the archive root. When a root copy exists, any nested copy is an
//!   error. (Legacy `data/` archives without a root manifest may contain nested
//!   `manifest.json` files as ordinary payload; those are ignored.)
//! - `*.pkg.lua` / `*.pkg.lua.sig` are only considered at the archive root.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use anyhow::{Result, anyhow};
use zoi_core::hash::{HashAlgorithm, calculate_reader_hash};
use zoi_core::pgp;
/// Key under which builds record the bundled `.pkg.lua` digest inside the
/// pooled `manifest.json`.
pub use zoi_package::build::PKG_LUA_HASH_KEY;

/// The kind of a trust-relevant archive entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustEntryKind {
    /// The pooled `manifest.json`.
    Manifest,
    /// The embedded `manifest.sig`.
    ManifestSig,
    /// A `*.pkg.lua` package definition.
    PkgLua,
    /// A `*.pkg.lua.sig` embedded source-bundle signature.
    PkgLuaSig
}

/// Classifies an archive entry path.
///
/// Returns the kind and, when the entry sits at the archive root, its file
/// name. Paths are normalized the way extraction does it (root, prefix and
/// `.` components dropped). Entries containing `..` are never extracted and
/// are therefore not classified.
#[must_use]
pub fn classify_entry(path: &Path) -> Option<(TrustEntryKind, Option<String>)> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => return None,
            Component::Normal(part) => parts.push(part.to_string_lossy())
        }
    }
    let file_name = parts.last()?;
    let kind = if file_name == "manifest.json" {
        TrustEntryKind::Manifest
    } else if file_name == "manifest.sig" {
        TrustEntryKind::ManifestSig
    } else if file_name.ends_with(".pkg.lua.sig") {
        TrustEntryKind::PkgLuaSig
    } else if file_name.ends_with(".pkg.lua") {
        TrustEntryKind::PkgLua
    } else {
        return None;
    };
    let top_level = (parts.len() == 1).then(|| file_name.to_string());
    Some((kind, top_level))
}

/// Trust-relevant entries collected from an archive.
#[derive(Debug, Default)]
pub struct ArchiveTrustEntries {
    /// The root `manifest.json`, if any.
    pub manifest: Option<Vec<u8>>,
    /// The root `manifest.sig`, if any.
    pub manifest_sig: Option<Vec<u8>>,
    /// Root `*.pkg.lua` files by name.
    pub pkg_luas: BTreeMap<String, Vec<u8>>,
    /// Root `*.pkg.lua.sig` files by name.
    pub pkg_lua_sigs: BTreeMap<String, Vec<u8>>
}

impl ArchiveTrustEntries {
    /// Returns the single root `.pkg.lua` of a package archive.
    ///
    /// # Errors
    ///
    /// Returns an error when the archive has more than one root definition,
    /// which a built `.zpa` never does.
    pub fn single_pkg_lua(&self) -> Result<Option<(&str, &[u8])>> {
        if self.pkg_luas.len() > 1 {
            return Err(anyhow!(
                "Archive contains more than one top-level .pkg.lua file: {}",
                self.pkg_luas.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        Ok(self
            .pkg_luas
            .iter()
            .next()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice())))
    }
}

/// Accumulates trust-relevant entries while an archive is streamed and
/// enforces the uniqueness rules once the stream ends.
#[derive(Debug, Default)]
pub struct TrustScan {
    /// Root-level manifest, signature and `.pkg.lua` entries seen so far.
    entries: ArchiveTrustEntries,
    /// Number of `manifest.json`/`manifest.sig` entries below the archive
    /// root, which are rejected when a root manifest is also present.
    nested_manifest_entries: usize
}

impl TrustScan {
    /// Records one classified entry.
    ///
    /// `data` must hold the entry's bytes for root entries that are regular
    /// files; it is ignored otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error for a root trust entry that is not a regular file, or
    /// for a repeated root entry.
    pub fn record(
        &mut self,
        kind: TrustEntryKind,
        top_level_name: Option<&str>,
        is_regular_file: bool,
        data: Option<Vec<u8>>
    ) -> Result<()> {
        let Some(name) = top_level_name else {
            if matches!(
                kind,
                TrustEntryKind::Manifest | TrustEntryKind::ManifestSig
            ) {
                self.nested_manifest_entries += 1;
            }
            return Ok(());
        };
        if !is_regular_file {
            return Err(anyhow!(
                "Archive entry '{name}' must be a regular file"
            ));
        }
        let data = data.ok_or_else(|| {
            anyhow!("internal error: contents of '{name}' were not read")
        })?;
        let duplicate = match kind {
            TrustEntryKind::Manifest => {
                self.entries.manifest.replace(data).is_some()
            }
            TrustEntryKind::ManifestSig => {
                self.entries.manifest_sig.replace(data).is_some()
            }
            TrustEntryKind::PkgLua => self
                .entries
                .pkg_luas
                .insert(name.to_string(), data)
                .is_some(),
            TrustEntryKind::PkgLuaSig => self
                .entries
                .pkg_lua_sigs
                .insert(name.to_string(), data)
                .is_some()
        };
        if duplicate {
            return Err(anyhow!(
                "Archive contains more than one '{name}' entry"
            ));
        }
        Ok(())
    }

    /// Finishes the scan.
    ///
    /// # Errors
    ///
    /// Returns an error when a root manifest or signature is accompanied by
    /// nested copies.
    pub fn finish(self) -> Result<ArchiveTrustEntries> {
        let has_root_manifest = self.entries.manifest.is_some()
            || self.entries.manifest_sig.is_some();
        if has_root_manifest && self.nested_manifest_entries > 0 {
            return Err(anyhow!(
                "Archive contains a nested manifest.json or manifest.sig next \
                 to its top-level manifest"
            ));
        }
        Ok(self.entries)
    }
}

/// Streams a `.zpa`/`.zsa` archive and collects its trust-relevant entries.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or breaks the uniqueness
/// rules described in the module documentation.
pub fn scan_archive(archive_path: &Path) -> Result<ArchiveTrustEntries> {
    let file = fs::File::open(archive_path)?;
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);
    let mut scan = TrustScan::default();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let Some((kind, top_level)) = classify_entry(&path) else {
            continue;
        };
        let is_file = entry.header().entry_type().is_file();
        let data = if top_level.is_some() && is_file {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            Some(buf)
        } else {
            None
        };
        scan.record(kind, top_level.as_deref(), is_file, data)?;
    }
    scan.finish()
}

/// Returns the lowercase hex SHA-256 of `bytes`.
///
/// # Errors
///
/// Returns an error if hashing fails.
pub fn sha256_hex(bytes: &[u8]) -> Result<String> {
    Ok(calculate_reader_hash(
        &mut std::io::Cursor::new(bytes),
        HashAlgorithm::Sha256
    )?
    .1)
}

/// Returns the `.pkg.lua` digest recorded in a pooled manifest, if any.
///
/// Manifests that are not JSON objects simply record nothing.
#[must_use]
pub fn recorded_pkg_lua_hash(manifest_bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value =
        serde_json::from_slice(manifest_bytes).ok()?;
    value
        .get(PKG_LUA_HASH_KEY)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Checks a package definition against the digest recorded in the manifest.
///
/// Returns `Ok(true)` when the manifest binds the definition and it matches,
/// `Ok(false)` when the manifest records no digest (archives built before
/// the digest existed).
///
/// # Errors
///
/// Returns an error when a digest is recorded but the definition is missing
/// or does not match it.
pub fn check_pkg_lua_binding(
    manifest_bytes: &[u8],
    pkg_lua: Option<&[u8]>
) -> Result<bool> {
    let Some(expected) = recorded_pkg_lua_hash(manifest_bytes) else {
        return Ok(false);
    };
    let pkg_lua = pkg_lua.ok_or_else(|| {
        anyhow!(
            "Archive manifest records a .pkg.lua digest but the archive has \
             no top-level .pkg.lua"
        )
    })?;
    let expected_hex = expected.strip_prefix("sha256-").unwrap_or(&expected);
    let actual = sha256_hex(pkg_lua)?;
    if !expected_hex.eq_ignore_ascii_case(&actual) {
        return Err(anyhow!(
            "Bundled .pkg.lua does not match the digest recorded in the \
             archive manifest (expected sha256-{expected_hex}, got \
             sha256-{actual})"
        ));
    }
    Ok(true)
}

/// Verifies a detached signature over in-memory data against any of the
/// trusted certificates.
fn verify_bytes(
    data: &[u8],
    signature: &[u8],
    trusted_certs: Vec<pgp::sequoia_openpgp::Cert>
) -> Result<()> {
    // Reuse the path-based multi-key verifier on temporary files so the exact
    // bytes that ship inside the archive are what gets verified.
    let temp_dir = tempfile::Builder::new()
        .prefix("zoi-embed-sig-")
        .tempdir()?;
    let data_path = temp_dir.path().join("data");
    let sig_path = temp_dir.path().join("data.sig");
    fs::write(&data_path, data)?;
    fs::write(&sig_path, signature)?;
    pgp::verify_detached_signature_multi_key(
        &data_path,
        &sig_path,
        trusted_certs
    )
}

/// Verifies an embedded `manifest.sig` inside a pooled `.zpa` archive.
///
/// When the signed manifest records a `.pkg.lua` digest, the bundled
/// definition is checked against it too, so the signature also covers the
/// Lua code the installer executes.
///
/// Returns `Ok(false)` when the archive carries no embedded signature pair,
/// letting the caller fall back to a detached signature.
///
/// # Errors
///
/// Returns an error if the archive cannot be read, is ambiguous, the
/// signature does not verify, or the definition does not match its digest.
pub fn verify_embedded_manifest_signature(
    archive_path: &Path,
    trusted_certs: Vec<pgp::sequoia_openpgp::Cert>
) -> Result<bool> {
    let entries = scan_archive(archive_path)?;
    let (Some(manifest), Some(sig)) =
        (&entries.manifest, &entries.manifest_sig)
    else {
        return Ok(false);
    };
    verify_bytes(manifest, sig, trusted_certs)?;
    let pkg_lua = entries.single_pkg_lua()?.map(|(_, bytes)| bytes);
    check_pkg_lua_binding(manifest, pkg_lua)?;
    Ok(true)
}

/// Verifies the embedded `<name>.pkg.lua.sig` signatures of a `.zsa` source
/// bundle.
///
/// Every root signature must have its root definition beside it and verify
/// against it. Returns the name of the verified definition (the first by
/// name when several are signed, matching
/// `zoi_package::bundle::select_bundle_pkg_lua`), or `None` when the bundle
/// carries no embedded signature.
///
/// Note that the signature covers the package definition only, not the
/// sources and assets bundled next to it.
///
/// # Errors
///
/// Returns an error if the bundle cannot be read, is ambiguous, or a
/// signature does not verify.
pub fn verify_embedded_bundle_signature(
    bundle_path: &Path,
    trusted_certs: &[pgp::sequoia_openpgp::Cert]
) -> Result<Option<String>> {
    let entries = scan_archive(bundle_path)?;
    let mut verified = None;
    for (sig_name, sig) in &entries.pkg_lua_sigs {
        let pkg_name = sig_name.strip_suffix(".sig").unwrap_or(sig_name);
        let pkg_lua = entries.pkg_luas.get(pkg_name).ok_or_else(|| {
            anyhow!(
                "Bundle signature '{sig_name}' has no matching '{pkg_name}'"
            )
        })?;
        verify_bytes(pkg_lua, sig, trusted_certs.to_vec())?;
        if verified.is_none() {
            verified = Some(pkg_name.to_string());
        }
    }
    Ok(verified)
}
