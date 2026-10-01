use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use colored::Colorize;
use dirs;
use hex;
use indicatif::{ProgressBar, ProgressStyle};
use self_replace;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha512};
use tar::Archive;
use tempfile::Builder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// The GitLab project path for the Zoi repository.
const GITLAB_PROJECT_PATH: &str = "zillowe/zillwen/zusty/zoi";
/// The GitLab project ID for the Zoi repository.
const GITLAB_PROJECT_ID: &str = "71087662";

/// Represents a release from the GitLab API.
#[derive(Debug, Deserialize)]
struct GitLabRelease {
    /// The tag name of the release.
    tag_name: String,
    /// When the release was created, used to pick the newest match.
    #[serde(default)]
    created_at: String
}

/// Branch identifiers, as used in release tags and the `--branch` flag.
///
/// ZFVM defines the vocabulary in the `zfvm` crate. The list is duplicated
/// here only so the error message can name the valid options without
/// formatting a borrowed slice of the dependency.
const BRANCH_NAMES: [&str; 4] = ["Prod", "Dev", "Spec", "Pub"];

/// Projects a ZFVM version onto the `SemVer` string used for the bsdiff patch
/// naming that `scripts/archive.sh` writes.
///
/// The status becomes a numeric pre-release ordinal and the build becomes
/// build metadata, matching `scripts/archive.sh` and the `zfvm` crate. The
/// branch does not participate: `archive.sh` builds these names from the tag
/// alone, and any mismatch between the two sides silently disables delta
/// upgrades in favour of full downloads.
///
/// # Errors
///
/// Returns an error when `number` is not a valid `SemVer` version, or when the
/// projected string cannot be parsed back.
fn zfvm_to_semver(number: &str, status: &str) -> Result<String> {
    // - `stable` is not a ZFVM status. It is accepted here because tags
    // - predating the ZFVM vocabulary may carry it, and rejecting it would
    // - strand those installs on the full-download path.
    let parsed_status = if status.eq_ignore_ascii_case("stable") {
        zfvm::Status::Release
    } else {
        status
            .parse::<zfvm::Status>()
            .map_err(|e| anyhow!("Malformed ZFVM status '{status}': {e}"))?
    };

    let core = semver::Version::parse(number).map_err(|e| {
        anyhow!("Malformed ZFVM version number '{number}': {e}")
    })?;

    let version =
        zfvm::Version::new(zfvm::Branch::Prod, parsed_status, core, None)
            .map_err(|e| anyhow!("Cannot build ZFVM version: {e}"))?;

    // Round-trip through the parser so this function can never return a
    // string the upgrade comparison would fail to parse.
    let projected = version.to_semver();
    semver::Version::parse(&projected).map_err(|e| {
        anyhow!("ZFVM projection '{projected}' is not valid SemVer: {e}")
    })?;

    Ok(projected)
}

/// Fetches the latest tag from GitLab for a given branch prefix.
fn get_latest_tag(branch_prefix: &str) -> Result<String> {
    println!("Fetching latest release information from GitLab...");
    let api_url = format!(
        "https://gitlab.com/api/v4/projects/{GITLAB_PROJECT_ID}/releases"
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("Zoi-Upgrader")
        .use_rustls_tls()
        .build()?;
    let releases: Vec<GitLabRelease> = client.get(&api_url).send()?.json()?;

    // - Releases are returned newest-first by the GitLab API, but the ordering
    // - is not contractual. Sort by `created_at` so "latest" means latest even
    // - if the API changes its default ordering.
    let latest_tag = releases
        .into_iter()
        .filter(|r| r.tag_name.starts_with(branch_prefix))
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .map(|r| r.tag_name)
        .ok_or_else(|| {
            anyhow!("No release found with prefix '{branch_prefix}'")
        })?;

    println!(
        "Found latest tag for branch prefix '{}': {}",
        branch_prefix,
        latest_tag.green()
    );
    Ok(latest_tag)
}

/// Downloads a file from a URL to a local path with a progress bar.
fn download_file(url: &str, path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download file: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                 {bytes}/{total_bytes} ({bytes_per_sec})"
            )?
            .progress_chars("#>- ")
    );

    let mut dest = File::create(path)?;
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        dest.write_all(
            buffer
                .get(..bytes_read)
                .ok_or_else(|| anyhow!("Buffer overflow during write"))?
        )?;
        pb.inc(bytes_read as u64);
    }

    pb.finish_with_message("Download complete.");
    Ok(())
}

/// Extracts a zip or zstd-compressed tar archive to a target directory.
fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<()> {
    println!("Extracting binary...");
    let file = File::open(archive_path)?;

    if archive_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        let mut archive = ZipArchive::new(file)?;
        archive.extract(target_dir)?;
    } else {
        let tar = ZstdDecoder::new(file)?;
        let mut archive = Archive::new(tar);
        archive.unpack(target_dir)?;
    }
    Ok(())
}

/// Verifies the SHA-512 checksum of a file against expected content.
fn verify_checksum(
    file_path: &Path,
    checksums_content: &str,
    filename: &str
) -> Result<()> {
    println!("Verifying checksum for {filename}...");
    let expected_hash = checksums_content
        .lines()
        .find(|line| line.contains(filename))
        .and_then(|line| line.split_whitespace().next())
        .ok_or(anyhow!("Checksum not found for {filename}."))?;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = io::Read::read(&mut file, &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..bytes_read)
                .ok_or_else(|| anyhow!("Buffer overflow during hash update"))?
        );
    }
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(anyhow!(
            "Checksum mismatch for {filename}! The file may be corrupt."
        ));
    }
    println!("Checksum verified successfully for {}.", filename.green());
    Ok(())
}

/// Returns the current platform's OS and architecture labels used in release
/// filenames.
fn get_platform_info() -> Result<(&'static str, &'static str)> {
    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" | "darwin" => "macos",
        "windows" => "windows",
        _ => return Err(anyhow!("Unsupported OS: {}", env::consts::OS))
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => {
            return Err(anyhow!(
                "Unsupported architecture: {}",
                env::consts::ARCH
            ));
        }
    };
    Ok((os, arch))
}

/// Performs a full upgrade by downloading the entire binary archive.
fn fallback_full_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str
) -> Result<(PathBuf, tempfile::TempDir)> {
    let archive_ext = if os == "windows" { "zip" } else { "tar.zst" };
    let archive_filename = format!("zoi-{os}-{arch}.{archive_ext}");
    let download_url = format!("{base_url}/{archive_filename}");
    let temp_dir = Builder::new().prefix("zoi-full-upgrade").tempdir()?;
    let temp_archive_path = temp_dir.path().join(&archive_filename);

    println!("Downloading Zoi from: {download_url}");
    download_file(&download_url, &temp_archive_path)?;
    verify_checksum(&temp_archive_path, checksums_content, &archive_filename)?;

    extract_archive(&temp_archive_path, temp_dir.path())?;

    let binary_filename = if os == "windows" { "zoi.exe" } else { "zoi" };
    let new_binary_path = temp_dir.path().join(binary_filename);
    if !new_binary_path.exists() {
        return Err(anyhow!(
            "Could not find executable in the extracted archive."
        ));
    }
    Ok((new_binary_path, temp_dir))
}

/// Attempts a delta upgrade by downloading a bsdiff patch.
fn try_delta_upgrade(
    base_url: &str,
    checksums_content: &str,
    os: &str,
    arch: &str,
    current_version: &str,
    latest_version: &str
) -> Result<(PathBuf, tempfile::TempDir)> {
    let archive_basename = format!("zoi-{os}-{arch}");
    let bsdiff_filename = format!(
        "{archive_basename}.from-v{current_version}-to-v{latest_version}.\
         bsdiff"
    );
    let download_url = format!("{base_url}/{bsdiff_filename}");

    if !checksums_content.contains(&bsdiff_filename) {
        return Err(anyhow!(
            "Delta patch not available for this upgrade path."
        ));
    }

    let temp_dir = Builder::new().prefix("zoi-delta-upgrade").tempdir()?;
    let temp_patch_path = temp_dir.path().join(&bsdiff_filename);

    println!("{} Downloading delta patch...", "::".bold().blue());
    download_file(&download_url, &temp_patch_path)?;
    verify_checksum(&temp_patch_path, checksums_content, &bsdiff_filename)?;

    println!("{} Applying delta patch...", "::".bold().blue());
    let current_exe_path = env::current_exe()?;
    let mut old_binary = Vec::new();
    File::open(&current_exe_path)?.read_to_end(&mut old_binary)?;

    let mut patch_data = Vec::new();
    File::open(&temp_patch_path)?.read_to_end(&mut patch_data)?;

    let raw_patch = if patch_data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        let mut decoder = ZstdDecoder::new(std::io::Cursor::new(&patch_data))?;
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        buf
    } else {
        patch_data
    };

    let mut new_binary = Vec::new();
    zbsdiff::Bspatch::new(&raw_patch)?
        .apply(&old_binary, std::io::Cursor::new(&mut new_binary))?;

    let binary_filename = if os == "windows" { "zoi.exe" } else { "zoi" };
    let new_binary_path = temp_dir.path().join(binary_filename);
    std::fs::write(&new_binary_path, &new_binary)?;

    Ok((new_binary_path, temp_dir))
}

/// Runs the upgrade process for Zoi.
///
/// # Errors
///
/// Returns an error if:
/// - Zoi is in offline mode.
/// - The GitLab API cannot be reached.
/// - The download fails or the checksum is invalid.
/// - The binary replacement fails.
pub fn run(
    branch: &str,
    status: &str,
    number: &str,
    force: bool,
    tag: Option<String>,
    custom_branch: Option<String>
) -> Result<()> {
    if crate::offline::is_offline() {
        return Err(anyhow!("Cannot upgrade Zoi: Zoi is in offline mode."));
    }
    let current_exe_path = env::current_exe()?;
    let path_str = current_exe_path.to_string_lossy();

    let is_cargo_install = dirs::home_dir().is_some_and(|home| {
        current_exe_path.starts_with(home.join(".cargo").join("bin"))
    });

    let pkg_manager = if path_str.contains("/Cellar/") {
        Some("Homebrew")
    } else if path_str.contains("scoop/apps/") {
        Some("Scoop")
    } else if path_str.starts_with("/usr/bin/") {
        Some("a system package manager")
    } else if is_cargo_install {
        Some("Cargo")
    } else {
        None
    };

    if let Some(pm) = pkg_manager {
        if !force {
            eprintln!(
                "{}{}{}",
                "Warning: ".yellow().bold(),
                "It looks like Zoi was installed via ".yellow(),
                pm.yellow().bold()
            );
            eprintln!(
                "{}",
                "Using 'zoi upgrade' may conflict with your package manager."
                    .yellow()
            );
            let upgrade_command = match pm {
                "Homebrew" => "brew upgrade zoi",
                "Scoop" => "scoop update zoi",
                "Cargo" => "cargo install zoi-rs",
                _ => "your package manager's upgrade command"
            };
            eprintln!(
                "It is recommended to use '{}' to upgrade Zoi.",
                upgrade_command.cyan()
            );
            eprintln!(
                "To override this check and proceed anyway, run with the '{}' \
                 flag.",
                "--force".cyan()
            );
            return Err(anyhow!("managed_by_package_manager"));
        }

        println!(
            "{}{}",
            "Warning: ".yellow().bold(),
            "Forcing self-upgrade on a package-manager-controlled \
             installation."
                .yellow()
        );
    }

    // - The branch is deliberately excluded from both projections. Cargo
    // - versions carry a branch suffix (`1.29.0-dev`) while ZFVM keeps branch
    // - and status orthogonal, so the comparison runs on status and number
    // - only. This matches the naming in `scripts/archive.sh`.
    let current_version = zfvm_to_semver(number, status)?;

    let latest_tag = if let Some(tag_name) = tag {
        println!("Upgrading to specified tag: {}", tag_name.green());
        tag_name
    } else {
        // - A custom `--branch` value must be a real ZFVM branch identifier,
        // - otherwise the tag lookup silently finds nothing.
        let branch_prefix = if let Some(b) = custom_branch {
            let canonical = BRANCH_NAMES
                .iter()
                .find(|known| known.eq_ignore_ascii_case(b.trim()))
                .ok_or_else(|| {
                    anyhow!(
                        "Unknown branch '{}'. Expected one of: {}",
                        b,
                        BRANCH_NAMES.join(", ")
                    )
                })?;
            println!(
                "Upgrading to latest release from branch: {}",
                canonical.green()
            );
            format!("{canonical}-")
        } else if branch.eq_ignore_ascii_case("Pub")
            || branch.eq_ignore_ascii_case("Public")
        {
            "Pub-".to_string()
        } else {
            "Prod-".to_string()
        };
        get_latest_tag(&branch_prefix)?
    };

    // - Parse through the `zfvm` crate rather than splitting on `-`. A
    // - positional split of `Prod-Pre-Alpha-0.1.0` yields status `pre` and
    // - core `Alpha`, neither of which is a valid ZFVM value.
    let latest = zfvm::symbolic::parse_long(&latest_tag)
        .map_err(|e| anyhow!("Malformed release tag '{latest_tag}': {e}"))?;
    let latest_version_str =
        zfvm_to_semver(&latest.core().to_string(), latest.status().as_str())?;

    if !force
        && (Version::parse(&latest_version_str)?
            <= Version::parse(&current_version)?)
    {
        println!(
            "
{}",
            "You are already on the latest version!".green()
        );
        return Err(anyhow!("already_on_latest"));
    }

    let (os, arch) = get_platform_info()?;

    let base_url = format!(
        "https://gitlab.com/{GITLAB_PROJECT_PATH}/-/releases/{latest_tag}/downloads"
    );
    let checksums_txt_url = format!("{base_url}/checksums.txt");

    println!("Downloading archive and checksums from: {checksums_txt_url}");
    let checksums_txt_content =
        reqwest::blocking::get(&checksums_txt_url)?.text()?;

    let (new_binary_path, _temp_dir_guard) = if force {
        fallback_full_upgrade(&base_url, &checksums_txt_content, os, arch)?
    } else {
        match try_delta_upgrade(
            &base_url,
            &checksums_txt_content,
            os,
            arch,
            &current_version,
            &latest_version_str
        ) {
            Ok(res) => res,
            Err(e) => {
                println!(
                    "Delta upgrade failed: {e}. Falling back to full upgrade."
                );
                fallback_full_upgrade(
                    &base_url,
                    &checksums_txt_content,
                    os,
                    arch
                )?
            }
        }
    };

    println!("Replacing current executable...");
    self_replace::self_replace(&new_binary_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::zfvm_to_semver;

    /// Projects a version the way `archive.sh` names its bsdiff patches.
    fn project(number: &str, status: &str) -> String {
        zfvm_to_semver(number, status).expect("fixture must project")
    }

    // ZFVM statuses are case-sensitive, so only the exact spelling `Release`
    // projects to a bare version.
    #[test]
    fn release_status_projects_to_a_bare_version() {
        assert_eq!(project("1.29.0", "Release"), "1.29.0");
    }

    // `stable` is not a ZFVM status. It predates the vocabulary and is
    // accepted case-insensitively so those tags keep resolving.
    #[test]
    fn legacy_stable_status_projects_to_a_bare_version() {
        assert_eq!(project("1.29.0", "stable"), "1.29.0");
        assert_eq!(project("1.29.0", "Stable"), "1.29.0");
        assert_eq!(project("1.29.0", "STABLE"), "1.29.0");
    }

    // Every other status becomes a numeric pre-release ordinal, matching the
    // `zfvm` crate and `archive.sh`. A readable label would sort wrongly
    // against the others.
    #[test]
    fn other_statuses_project_to_ordinals() {
        assert_eq!(project("0.1.0", "Pre-Alpha"), "0.1.0-0");
        assert_eq!(project("1.29.0", "Alpha"), "1.29.0-1");
        assert_eq!(project("1.29.0", "Beta"), "1.29.0-2");
        assert_eq!(project("1.29.0", "RC"), "1.29.0-3");
    }

    // Every status must survive the projection and still parse as SemVer,
    // which is what the upgrade comparison and the bsdiff naming rely on.
    #[test]
    fn every_status_projects_to_parseable_semver() {
        for status in ["Pre-Alpha", "Alpha", "Beta", "RC", "Release"] {
            let number = if status == "Release" {
                "1.29.0"
            } else {
                "0.1.0"
            };
            let projected = project(number, status);
            assert!(
                semver::Version::parse(&projected).is_ok(),
                "{status} projected to unparseable {projected}"
            );
        }
    }

    #[test]
    fn rejects_unknown_status() {
        assert!(zfvm_to_semver("1.29.0", "nightly").is_err());
        assert!(zfvm_to_semver("1.29.0", "Prealpha").is_err());
        // Case-sensitive: only the exact ZFVM spelling is accepted.
        assert!(zfvm_to_semver("1.29.0", "release").is_err());
        assert!(zfvm_to_semver("1.29.0", "beta").is_err());
    }

    #[test]
    fn rejects_malformed_version_number() {
        assert!(zfvm_to_semver("1.29", "Release").is_err());
        assert!(zfvm_to_semver("1.29.0-rc.1", "Release").is_err());
        assert!(zfvm_to_semver("", "Release").is_err());
    }
}
