//! Containerized package builds using Docker.
//!
//! This module allows building Zoi packages inside a Docker container.
//! This is useful for cross-compilation, ensuring a clean and consistent
//! build environment, and for building packages for different Linux
//! distributions from a single host.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Result, anyhow};
use colored::Colorize;
use zoi_core::{types, utils};

/// Runs the package build process inside a Docker container.
/// # Errors
///
/// Returns an error if the Docker image cannot be built or the container fails
/// to run.
pub fn run(
    package_file: &Path,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    sign_mode: types::SignMode,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    image: &str,
    fakeroot: bool,
    install_deps: bool,
    test: bool
) -> Result<()> {
    println!("{} Building package using Docker...", "::".bold().blue());
    println!("Image: {}", image.cyan());

    if !utils::command_exists("docker") {
        return Err(anyhow!(
            "Docker is not installed or not in PATH. Please install Docker to \
             use this method."
        ));
    }

    let abs_package_file = package_file.canonicalize()?;
    let package_dir = abs_package_file.parent().ok_or_else(|| {
        anyhow!("Could not get parent directory of package file")
    })?;

    let abs_output_dir = if let Some(dir) = output_dir {
        if !dir.exists() {
            std::fs::create_dir_all(dir)?;
        }
        dir.canonicalize()?
    } else {
        package_dir.to_path_buf()
    };

    let container_workdir = "/work";
    let container_output_dir = "/output";

    let mut docker_args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-v".to_string(),
        format!("{}:{}", package_dir.display(), container_workdir),
        "-v".to_string(),
        format!("{}:{}", abs_output_dir.display(), container_output_dir),
        "-w".to_string(),
        container_workdir.to_string(),
    ];

    if let Ok(user_id) = Command::new("id").arg("-u").output() {
        let uid = String::from_utf8_lossy(&user_id.stdout).trim().to_string();
        if let Ok(group_id) = Command::new("id").arg("-g").output() {
            let gid =
                String::from_utf8_lossy(&group_id.stdout).trim().to_string();
            docker_args.push("--user".to_string());
            docker_args.push(format!("{uid}:{gid}"));
        }
    }

    // The host's GnuPG home and GPG_PASSWORD are deliberately *not* passed
    // into the container: the build runs untrusted package code. Archives are
    // signed on the host after the container exits instead.
    let archives_before = snapshot_archives(&abs_output_dir);

    docker_args.push(image.to_string());

    let package_filename = abs_package_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid package file name"))?
        .to_string_lossy()
        .into_owned();
    let build_args = build_command_args(
        package_filename,
        build_type,
        platforms,
        container_output_dir,
        version_override,
        sub_packages,
        fakeroot,
        install_deps,
        test
    );

    let inner_script = "if ! command -v sudo >/dev/null 2>&1 && [ \"$(id -u)\" -eq 0 ]; then \
            if command -v pacman >/dev/null 2>&1; then pacman -Sy --noconfirm sudo gnupg; \
            elif command -v apt-get >/dev/null 2>&1; then apt-get update && apt-get install -y sudo gnupg; \
            elif command -v dnf >/dev/null 2>&1; then dnf install -y sudo gnupg; \
            elif command -v apk >/dev/null 2>&1; then apk add --update sudo gnupg; fi; \
         fi && \
         if command -v pacman >/dev/null 2>&1; then pacman -Sy --noconfirm base-devel git; \
         elif command -v apt-get >/dev/null 2>&1; then apt-get update && apt-get install -y build-essential git; \
         elif command -v dnf >/dev/null 2>&1; then dnf install -y @development-tools git; \
         elif command -v apk >/dev/null 2>&1; then apk add --update build-base git; fi && \
         curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash && \
         export PATH=\"$HOME/.local/bin:$PATH\" && \
         zoi sync && \
         exec zoi package build \"$@\"";

    docker_args.push("bash".to_string());
    docker_args.push("-c".to_string());
    docker_args.push(inner_script.to_string());
    docker_args.push("zoi-package-build".to_string());
    docker_args.extend(build_args);

    println!("Running docker command: {}", "docker".cyan());
    let status = Command::new("docker")
        .args(&docker_args)
        .env_remove("GPG_PASSWORD")
        .status()?;

    if !status.success() {
        return Err(anyhow!(
            "Docker build failed with exit code {:?}",
            status.code()
        ));
    }

    println!("{}", "Docker build successful!".green());

    if let Some(key_id) = sign_key {
        sign_new_archives(
            &abs_output_dir,
            &archives_before,
            &key_id,
            sign_mode
        )?;
    }

    Ok(())
}

/// Records the modification time of every `.zpa` archive in `dir`.
///
/// Used together with [`sign_new_archives`] to find the archives an isolated
/// (Docker or Bubblewrap) build produced.
pub(crate) fn snapshot_archives(dir: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut snapshot = HashMap::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_zpa(&path)
                && let Ok(modified) =
                    entry.metadata().and_then(|meta| meta.modified())
            {
                snapshot.insert(path, modified);
            }
        }
    }
    snapshot
}

/// Returns `true` if `path` names a `.zpa` package archive.
///
/// Only regular files count: the output directory is writable by the
/// isolated build, so a symlink there must never be followed by the host.
fn is_zpa(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zpa"))
}

/// Signs, on the host, every `.zpa` archive in `dir` that is new or was
/// modified since `before` was taken.
///
/// The isolated build never sees the signing key or its passphrase. With
/// [`types::SignMode::File`] a detached `<archive>.sig` is written next to
/// each archive. With [`types::SignMode::Embed`] the archive's root
/// `manifest.json` is signed and the signature is appended to the archive as
/// `manifest.sig`, after which the `.hash` and `.size` sidecars are refreshed.
///
/// # Errors
///
/// Returns an error if no archive was produced or signing any archive fails.
pub(crate) fn sign_new_archives(
    dir: &Path,
    before: &HashMap<PathBuf, SystemTime>,
    key_id: &str,
    sign_mode: types::SignMode
) -> Result<()> {
    let mut produced: Vec<PathBuf> = snapshot_archives(dir)
        .into_iter()
        .filter(|(path, modified)| {
            before.get(path).is_none_or(|previous| modified > previous)
        })
        .map(|(path, _)| path)
        .collect();
    produced.sort();

    if produced.is_empty() {
        return Err(anyhow!(
            "Signing was requested but the isolated build produced no .zpa \
             archives in {}.",
            dir.display()
        ));
    }

    for archive in produced {
        match sign_mode {
            types::SignMode::File => {
                println!(
                    "Signing {} with key '{}'...",
                    archive.display(),
                    key_id.cyan()
                );
                let signature_path =
                    PathBuf::from(format!("{}.sig", archive.display()));
                // Remove whatever the build left there, including a
                // (possibly dangling) symlink gpg would otherwise write
                // through.
                remove_existing_entry(&signature_path)?;
                zoi_core::pgp::sign_detached(
                    &archive,
                    &signature_path,
                    key_id
                )?;
            }
            types::SignMode::Embed => {
                println!(
                    "Embedding manifest signature into {} with key '{}'...",
                    archive.display(),
                    key_id.cyan()
                );
                embed_manifest_signature(&archive, key_id)?;
            }
        }
    }
    println!("{}", "Archives signed on the host.".green());
    Ok(())
}

/// Returns the root-level file name of an archive entry path, ignoring `.`
/// components, or `None` for nested entries.
fn root_entry_name(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => {
                parts.push(part.to_string_lossy().into_owned());
            }
            _ => return None
        }
    }
    if parts.len() == 1 { parts.pop() } else { None }
}

/// Signs the root `manifest.json` of `archive` and rewrites the archive with
/// an added root `manifest.sig` entry, then refreshes the `.hash` and `.size`
/// sidecars.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or rewritten, has no root
/// `manifest.json`, already carries a `manifest.sig`, or signing fails.
fn embed_manifest_signature(archive: &Path, key_id: &str) -> Result<()> {
    use std::io::Read as _;

    let parent = archive
        .parent()
        .ok_or_else(|| anyhow!("Archive has no parent directory"))?;

    // Pass 1: extract the root manifest.
    let mut manifest: Option<Vec<u8>> = None;
    {
        let decoder = zstd::stream::read::Decoder::new(File::open(archive)?)?;
        let mut tar = tar::Archive::new(decoder);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let name = root_entry_name(&entry.path()?);
            match name.as_deref() {
                Some("manifest.sig") => {
                    return Err(anyhow!(
                        "{} already contains a manifest.sig entry.",
                        archive.display()
                    ));
                }
                Some("manifest.json")
                    if entry.header().entry_type().is_file() =>
                {
                    if manifest.is_some() {
                        return Err(anyhow!(
                            "{} contains more than one root manifest.json.",
                            archive.display()
                        ));
                    }
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data)?;
                    manifest = Some(data);
                }
                _ => {}
            }
        }
    }
    let manifest = manifest.ok_or_else(|| {
        anyhow!("{} has no root manifest.json to sign.", archive.display())
    })?;

    let work = tempfile::Builder::new()
        .prefix(".zoi-sign-")
        .tempdir_in(parent)?;
    let manifest_path = work.path().join("manifest.json");
    let signature_path = work.path().join("manifest.sig");
    fs::write(&manifest_path, &manifest)?;
    zoi_core::pgp::sign_detached(&manifest_path, &signature_path, key_id)?;
    let signature = fs::read(&signature_path)?;

    // Pass 2: copy every entry into a new archive and append manifest.sig.
    let rewritten = work.path().join("rewritten.zpa");
    {
        let decoder = zstd::stream::read::Decoder::new(File::open(archive)?)?;
        let mut source = tar::Archive::new(decoder);
        let encoder =
            zstd::stream::write::Encoder::new(File::create(&rewritten)?, 0)?
                .auto_finish();
        let mut builder = tar::Builder::new(encoder);

        for entry in source.entries()? {
            let entry = entry?;
            let path = entry.path()?.into_owned();
            let mut header = entry.header().clone();
            let entry_type = header.entry_type();
            if entry_type.is_symlink() || entry_type.is_hard_link() {
                let target = entry
                    .link_name()?
                    .ok_or_else(|| {
                        anyhow!("Link entry {} has no target", path.display())
                    })?
                    .into_owned();
                builder.append_link(&mut header, &path, &target)?;
            } else {
                builder.append_data(&mut header, &path, entry)?;
            }
        }

        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_size(signature.len() as u64);
        header.set_mtime(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        );
        builder.append_data(
            &mut header,
            "manifest.sig",
            signature.as_slice()
        )?;
        builder.into_inner()?;
    }

    fs::rename(&rewritten, archive)?;
    refresh_sidecars(archive, signature.len() as u64)?;
    Ok(())
}

/// Rewrites the `.hash` sidecar and the `down:`/`install:` values of the
/// `.size` sidecar after an archive was modified on the host.
///
/// # Errors
///
/// Returns an error if the archive cannot be hashed or a sidecar cannot be
/// written.
fn refresh_sidecars(archive: &Path, added_bytes: u64) -> Result<()> {
    let file_name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Archive file name is not valid UTF-8"))?;

    let hash = zoi_core::hash::calculate_file_hash(
        archive,
        zoi_core::hash::HashAlgorithm::Sha512
    )?;
    replace_file_contents(
        &PathBuf::from(format!("{}.hash", archive.display())),
        format!("{hash}  {file_name}\n").as_bytes()
    )?;

    let size_path = PathBuf::from(format!("{}.size", archive.display()));
    let compressed = fs::metadata(archive)?.len();
    // Only trust a previous `.size` that is a regular file; never follow a
    // symlink the build may have left in the output directory.
    let previous_install = fs::symlink_metadata(&size_path)
        .ok()
        .filter(|meta| meta.file_type().is_file())
        .and_then(|_| fs::read_to_string(&size_path).ok())
        .and_then(|content| {
            content.lines().find_map(|line| {
                line.strip_prefix("install:")
                    .and_then(|v| v.trim().parse::<u64>().ok())
            })
        })
        .unwrap_or(0);
    replace_file_contents(
        &size_path,
        format!(
            "down: {compressed}\ninstall: {}\n",
            previous_install + added_bytes
        )
        .as_bytes()
    )?;
    Ok(())
}

/// Atomically replaces `path` with a new regular file holding `contents`.
///
/// The data goes to a fresh temporary file in the same directory, which is
/// then renamed over `path`. A rename replaces a symlink at `path` instead of
/// following it, so a link planted by the isolated build cannot redirect the
/// write to another host file.
///
/// # Errors
///
/// Returns an error if the temporary file cannot be created, written or
/// renamed into place.
fn replace_file_contents(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".zoi-sidecar-")
        .tempfile_in(parent)?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| anyhow!(e.error))?;
    Ok(())
}

/// Removes the directory entry at `path` without following it, so a symlink
/// is deleted rather than its target. A missing entry is not an error; a
/// real directory is refused.
///
/// # Errors
///
/// Returns an error if the entry is a directory or cannot be removed.
fn remove_existing_entry(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => Err(anyhow!(
            "{} is a directory, refusing to replace it",
            path.display()
        )),
        Ok(_) => Ok(fs::remove_file(path)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into())
    }
}

#[allow(clippy::too_many_arguments)]
/// Builds the argument vector forwarded by the Docker bootstrap shell.
///
/// Signing options are intentionally not forwarded: archives are signed on the
/// host after the container exits (see [`sign_new_archives`]).
fn build_command_args(
    package_filename: String,
    build_type: Option<&str>,
    platforms: &[String],
    output_dir: &str,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    fakeroot: bool,
    install_deps: bool,
    test: bool
) -> Vec<String> {
    let mut args = vec![
        package_filename,
        "--output-dir".to_string(),
        output_dir.to_string(),
    ];

    if let Some(build_type) = build_type {
        args.extend(["--type".to_string(), build_type.to_string()]);
    }
    for platform in platforms {
        args.extend(["--platform".to_string(), platform.clone()]);
    }
    if let Some(version_override) = version_override {
        args.extend([
            "--version-override".to_string(),
            version_override.to_string()
        ]);
    }
    if let Some(sub_packages) = sub_packages {
        for sub_package in sub_packages {
            args.extend(["--sub".to_string(), sub_package]);
        }
    }
    if fakeroot {
        args.push("--fakeroot".to_string());
    }
    if install_deps {
        args.push("--install-deps".to_string());
    }
    if test {
        args.push("--test".to_string());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::build_command_args;

    #[test]
    fn build_arguments_keep_shell_syntax_as_data() {
        let args = build_command_args(
            "package; touch /tmp/pwned.pkg.lua".to_string(),
            Some("source; id"),
            &["linux-amd64; id".to_string()],
            "/output",
            Some("1.0.0; id"),
            Some(vec!["sub; id".to_string()]),
            true,
            true,
            true
        );

        assert!(
            args.contains(&"package; touch /tmp/pwned.pkg.lua".to_string())
        );
        assert!(args.contains(&"source; id".to_string()));
        assert!(args.contains(&"linux-amd64; id".to_string()));
        assert!(args.contains(&"1.0.0; id".to_string()));
        assert!(args.contains(&"sub; id".to_string()));
    }

    #[test]
    fn signing_is_never_forwarded_into_the_container() {
        let args = build_command_args(
            "pkg.pkg.lua".to_string(),
            None,
            &[],
            "/output",
            None,
            None,
            false,
            false,
            false
        );
        assert!(!args.iter().any(|a| a == "--sign" || a == "--sign-mode"));
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_refresh_replaces_planted_symlinks_without_following_them() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let out = tempfile::tempdir().expect("tempdir");
        let host = tempfile::tempdir().expect("tempdir");
        let victim = host.path().join("victim");
        fs::write(&victim, "install: 7\nhost data\n").expect("write victim");

        let archive = out.path().join("pkg-1.0.0-linux-amd64.zpa");
        fs::write(&archive, b"archive bytes").expect("write archive");
        symlink(&victim, out.path().join("pkg-1.0.0-linux-amd64.zpa.hash"))
            .expect("symlink hash");
        symlink(&victim, out.path().join("pkg-1.0.0-linux-amd64.zpa.size"))
            .expect("symlink size");

        super::refresh_sidecars(&archive, 10).expect("refresh sidecars");

        assert_eq!(
            fs::read_to_string(&victim).expect("read victim"),
            "install: 7\nhost data\n",
            "the host file behind the symlink must be untouched"
        );
        for ext in ["hash", "size"] {
            let sidecar =
                out.path().join(format!("pkg-1.0.0-linux-amd64.zpa.{ext}"));
            let meta = fs::symlink_metadata(&sidecar).expect("sidecar meta");
            assert!(meta.file_type().is_file(), "{ext} must be a regular file");
        }
        // A symlinked `.size` is not trusted as the previous install size.
        let size = fs::read_to_string(
            out.path().join("pkg-1.0.0-linux-amd64.zpa.size")
        )
        .expect("read size");
        assert!(size.contains("install: 10\n"), "{size}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_archives_are_not_signed_and_stale_links_are_removed() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let out = tempfile::tempdir().expect("tempdir");
        let host = tempfile::tempdir().expect("tempdir");
        let target = host.path().join("real.zpa");
        fs::write(&target, b"host archive").expect("write target");

        let linked = out.path().join("linked.zpa");
        symlink(&target, &linked).expect("symlink archive");
        assert!(!super::is_zpa(&linked));
        assert!(super::snapshot_archives(out.path()).is_empty());

        let dangling = out.path().join("pkg.zpa.sig");
        symlink(host.path().join("missing"), &dangling).expect("symlink sig");
        super::remove_existing_entry(&dangling).expect("remove dangling link");
        assert!(fs::symlink_metadata(&dangling).is_err());
        super::remove_existing_entry(&dangling)
            .expect("a missing entry is not an error");
    }
}
