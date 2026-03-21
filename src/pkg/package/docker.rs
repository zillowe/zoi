use anyhow::{Result, anyhow};
use colored::*;
use std::path::Path;
use std::process::Command;

pub fn run(
    package_file: &Path,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    image: &str,
    install_deps: bool,
) -> Result<()> {
    println!("{} Building package using Docker...", "::".bold().blue());
    println!("Image: {}", image.cyan());

    if !crate::utils::command_exists("docker") {
        return Err(anyhow!(
            "Docker is not installed or not in PATH. Please install Docker to use this method."
        ));
    }

    let abs_package_file = package_file.canonicalize()?;
    let package_dir = abs_package_file
        .parent()
        .ok_or_else(|| anyhow!("Could not get parent directory of package file"))?;

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
            let gid = String::from_utf8_lossy(&group_id.stdout).trim().to_string();
            docker_args.push("--user".to_string());
            docker_args.push(format!("{}:{}", uid, gid));
        }
    }

    docker_args.push(image.to_string());

    let package_filename = abs_package_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid package file name"))?
        .to_string_lossy();

    let mut inner_cmd = format!(
        "if ! command -v sudo >/dev/null 2>&1 && [ \"$(id -u)\" -eq 0 ]; then \
            if command -v pacman >/dev/null 2>&1; then pacman -S --noconfirm sudo; \
            elif command -v apt-get >/dev/null 2>&1; then apt-get update && apt-get install -y sudo; \
            elif command -v dnf >/dev/null 2>&1; then dnf install -y sudo; \
            elif command -v apk >/dev/null 2>&1; then apk add sudo; fi; \
         fi && \
         curl -fsSL https://zillowe.pages.dev/scripts/zoi/install.sh | bash && \
         export PATH=\"$HOME/.local/bin:$PATH\" && \
         zoi sync && \
         zoi package build {} --output-dir {}",
        package_filename, container_output_dir
    );

    if let Some(bt) = build_type {
        inner_cmd.push_str(&format!(" --type {}", bt));
    }

    for p in platforms {
        inner_cmd.push_str(&format!(" --platform {}", p));
    }

    if let Some(sk) = sign_key {
        inner_cmd.push_str(&format!(" --sign {}", sk));
    }

    if let Some(v) = version_override {
        inner_cmd.push_str(&format!(" --version-override {}", v));
    }

    if let Some(subs) = sub_packages {
        for s in subs {
            inner_cmd.push_str(&format!(" --sub {}", s));
        }
    }

    if install_deps {
        inner_cmd.push_str(" --install-deps");
    }

    docker_args.push("bash".to_string());
    docker_args.push("-c".to_string());
    docker_args.push(inner_cmd);

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
