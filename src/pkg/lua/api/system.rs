use mlua::{self, Lua};

pub fn add_cmd_util(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let cmd_fn = lua.create_function(move |lua, command: String| {
        let build_dir: String = lua.globals().get("BUILD_DIR")?;

        if !quiet {
            println!("Executing: {}", command);
        }
        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("pwsh")
                .arg("-Command")
                .arg(&command)
                .current_dir(&build_dir)
                .output()
        } else {
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
                .current_dir(&build_dir)
                .output()
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let exit_code =
                    out.status
                        .code()
                        .unwrap_or(if out.status.success() { 0 } else { 1 });

                if !out.status.success() && !quiet {
                    eprintln!("[cmd] {}", stderr);
                }

                Ok((stdout, stderr, exit_code))
            }
            Err(e) => {
                if !quiet {
                    eprintln!("[cmd] Failed to execute command: {}", e);
                }
                Ok((String::new(), e.to_string(), 1))
            }
        }
    })?;
    lua.globals().set("cmd", cmd_fn)?;
    Ok(())
}
