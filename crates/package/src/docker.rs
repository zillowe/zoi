//! Containerized package builds using Docker.
//!
//! This module allows building Zoi packages inside a Docker container.
//! This is useful for cross-compilation, ensuring a clean and consistent
//! build environment, and for building packages for different Linux
//! distributions from a single host.

use std::path::{Path, PathBuf};
use std::process::Command;

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

    if sign_key.is_some() {
        let host_gpg_home = std::env::var("GNUPGHOME").map_or_else(
            |_| {
                utils::get_user_home()
                    .map(|h| h.join(".gnupg"))
                    .unwrap_or_default()
            },
            PathBuf::from
        );

        if host_gpg_home.exists() {
            let container_gpg_home = "/gpg_home";
            docker_args.push("-v".to_string());
            docker_args.push(format!(
                "{}:{}",
                host_gpg_home.display(),
                container_gpg_home
            ));
            docker_args.push("-e".to_string());
            docker_args.push(format!("GNUPGHOME={container_gpg_home}"));
        }
    }

    if let Ok(password) = std::env::var("GPG_PASSWORD") {
        docker_args.push("-e".to_string());
        docker_args.push(format!("GPG_PASSWORD={password}"));
    }

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
        sign_key,
        sign_mode,
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
    let status = Command::new("docker").args(&docker_args).status()?;

    if !status.success() {
        return Err(anyhow!(
            "Docker build failed with exit code {:?}",
            status.code()
        ));
    }

    println!("{}", "Docker build successful!".green());

    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Builds the argument vector forwarded by the Docker bootstrap shell.
fn build_command_args(
    package_filename: String,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    sign_mode: types::SignMode,
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
    if let Some(sign_key) = sign_key {
        args.extend(["--sign".to_string(), sign_key]);
        if sign_mode == types::SignMode::Embed {
            args.push("--sign-mode".to_string());
            args.push("embed".to_string());
        }
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
            Some("key; id".to_string()),
            zoi_core::types::SignMode::Embed,
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
        assert!(args.contains(&"key; id".to_string()));
        assert!(args.contains(&"1.0.0; id".to_string()));
        assert!(args.contains(&"sub; id".to_string()));
    }
}
