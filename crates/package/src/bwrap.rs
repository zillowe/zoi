use anyhow::{Result, anyhow};
use std::path::Path;

pub fn run(
    package_file: &Path,
    build_type: Option<&str>,
    platforms: &[String],
    sign_key: Option<String>,
    output_dir: Option<&Path>,
    version_override: Option<&str>,
    sub_packages: Option<Vec<String>>,
    fakeroot: bool,
    install_deps: bool,
    test: bool,
) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (
            package_file,
            build_type,
            platforms,
            sign_key,
            output_dir,
            version_override,
            sub_packages,
            fakeroot,
            install_deps,
            test,
        );
        return Err(anyhow!("Bubblewrap ('bwrap') is only supported on Linux."));
    }

    #[cfg(target_os = "linux")]
    {
        use colored::*;
        use std::path::PathBuf;
        use std::process::Command;
        use zoi_core::utils;

        println!(
            "{} Building package using Bubblewrap sandbox...",
            "::".bold().blue()
        );

        if !utils::command_exists("bwrap") {
            return Err(anyhow!(
                "Bubblewrap ('bwrap') is not installed or not in PATH. Please install it to use this method."
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

        // We use a temporary directory for the build inside the sandbox
        let container_workdir = "/work";
        let container_output_dir = "/output";

        let zoi_exe = std::env::current_exe()?;
        let zoi_exe_dir = zoi_exe
            .parent()
            .ok_or_else(|| anyhow!("Could not get zoi executable directory"))?;

        let home_dir = zoi_core::utils::get_user_home()
            .ok_or_else(|| anyhow!("Could not get home directory"))?;
        let zoi_home = home_dir.join(".zoi");

        let package_filename = abs_package_file
            .file_name()
            .ok_or_else(|| anyhow!("Invalid package file name"))?
            .to_string_lossy();

        // Construct the inner zoi command
        let mut inner_cmd = format!(
            "zoi package build {} --output-dir {} --method native",
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

        if fakeroot {
            inner_cmd.push_str(" --fakeroot");
        }

        if install_deps {
            inner_cmd.push_str(" --install-deps");
        }

        if test {
            inner_cmd.push_str(" --test");
        }

        // Base bwrap arguments
        let sysroot = zoi_core::sysroot::get_sysroot();

        let mut envs = std::collections::HashMap::new();
        envs.insert(
            "PATH".to_string(),
            "/zoi_bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
        );
        envs.insert("ZOI_SKIP_LOCK".to_string(), "1".to_string());
        envs.insert("HOME".to_string(), home_dir.display().to_string());

        let status = if let Some(root) = &sysroot {
            println!(
                "{} Isolated build using sysroot: {}",
                "::".bold().yellow(),
                root.display()
            );

            let extra_binds = vec![
                (package_dir.to_path_buf(), PathBuf::from(container_workdir)),
                (abs_output_dir.clone(), PathBuf::from(container_output_dir)),
                (zoi_exe_dir.to_path_buf(), PathBuf::from("/zoi_bin")),
            ];

            let mut cmd = zoi_sandbox::wrap_command_in_root(
                root,
                &PathBuf::from("/bin/bash"),
                &["-c".to_string(), inner_cmd],
                &envs,
                &extra_binds,
                fakeroot,
            )?;
            cmd.status()?
        } else {
            // Base bwrap arguments for non-sysroot build
            let mut bwrap_args = vec![
                "--unshare-all".to_string(),
                "--share-net".to_string(),
                "--hostname".to_string(),
                "zoi-build".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--tmpfs".to_string(),
                "/tmp".to_string(),
                "--tmpfs".to_string(),
                "/run".to_string(),
                "--tmpfs".to_string(),
                "/var".to_string(),
                "--ro-bind".to_string(),
                "/usr".to_string(),
                "/usr".to_string(),
                "--symlink".to_string(),
                "/usr/bin".to_string(),
                "/bin".to_string(),
                "--symlink".to_string(),
                "/usr/lib".to_string(),
                "/lib".to_string(),
                "--symlink".to_string(),
                "/usr/lib64".to_string(),
                "/lib64".to_string(),
                "--symlink".to_string(),
                "/usr/sbin".to_string(),
                "/sbin".to_string(),
                "--ro-bind".to_string(),
                "/etc".to_string(),
                "/etc".to_string(),
                "--bind".to_string(),
                package_dir.display().to_string(),
                container_workdir.to_string(),
                "--bind".to_string(),
                abs_output_dir.display().to_string(),
                container_output_dir.to_string(),
                "--ro-bind".to_string(),
                zoi_exe_dir.display().to_string(),
                "/zoi_bin".to_string(),
                "--chdir".to_string(),
                container_workdir.to_string(),
                "--setenv".to_string(),
                "PATH".to_string(),
                "/zoi_bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
                "--setenv".to_string(),
                "ZOI_SKIP_LOCK".to_string(),
                "1".to_string(),
            ];

            if zoi_home.exists() {
                bwrap_args.push("--bind".to_string());
                bwrap_args.push(zoi_home.display().to_string());
                bwrap_args.push(zoi_home.display().to_string());
            }

            let system_zoi = Path::new("/var/lib/zoi");
            if system_zoi.exists() {
                bwrap_args.push("--bind".to_string());
                bwrap_args.push(system_zoi.display().to_string());
                bwrap_args.push(system_zoi.display().to_string());
            }

            if fakeroot {
                bwrap_args.push("--uid".to_string());
                bwrap_args.push("0".to_string());
                bwrap_args.push("--gid".to_string());
                bwrap_args.push("0".to_string());
            } else {
                let uid = nix::unistd::getuid().as_raw();
                let gid = nix::unistd::getgid().as_raw();
                bwrap_args.push("--uid".to_string());
                bwrap_args.push(uid.to_string());
                bwrap_args.push("--gid".to_string());
                bwrap_args.push(gid.to_string());
            }

            bwrap_args.push("--setenv".to_string());
            bwrap_args.push("HOME".to_string());
            bwrap_args.push(home_dir.display().to_string());

            bwrap_args.push("bash".to_string());
            bwrap_args.push("-c".to_string());
            bwrap_args.push(inner_cmd);

            Command::new("bwrap").args(&bwrap_args).status()?
        };

        if !status.success() {
            return Err(anyhow!(
                "Bubblewrap build failed with exit code {:?}",
                status.code()
            ));
        }

        println!("{}", "Bubblewrap build successful!".green());

        Ok(())
    }
}
