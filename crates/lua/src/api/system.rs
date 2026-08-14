use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use mlua::{self, Lua, Table, Value};

/// Represents an active shell process used for executing commands from Lua.
struct InnerSession {
    /// The handle to the child shell process.
    child: Child,
    /// The standard input stream for the child process.
    stdin: ChildStdin,
    /// The buffered standard output stream for the child process.
    stdout: BufReader<ChildStdout>,
    /// The shared buffer for capturing standard error.
    stderr_buffer: Arc<Mutex<String>>,
    /// A unique string used to identify the end of a command's output.
    sentinel: String
}

impl Drop for InnerSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A wrapper around `InnerSession` providing thread-safe access.
struct ShellSession {
    /// The inner session state, protected by a mutex.
    inner: Mutex<InnerSession>
}

impl ShellSession {
    /// Creates a new `ShellSession` within the specified build directory.
    fn new(lua: &Lua, build_dir: &str) -> Result<Self, anyhow::Error> {
        let sentinel =
            format!("---ZOI_CMD_COMPLETE_{}---", uuid::Uuid::new_v4());

        let mut child = if cfg!(target_os = "windows") {
            Command::new("pwsh")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg("-")
                .current_dir(build_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        } else {
            Command::new("bash")
                .arg("--noprofile")
                .arg("--norc")
                .current_dir(build_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        };

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout =
            BufReader::new(child.stdout.take().expect("Failed to open stdout"));
        let stderr = child.stderr.take().expect("Failed to open stderr");

        let stderr_buffer = Arc::new(Mutex::new(String::new()));
        let buffer_clone = Arc::clone(&stderr_buffer);

        // Spawn a thread to consume stderr continuously
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if let Ok(mut buf) = buffer_clone.lock() {
                    buf.push_str(&line);
                }
                line.clear();
            }
        });

        // Inject Zoi environment variables
        let mut env_cmds = String::new();
        let globals = lua.globals();

        let vars = [
            ("BUILD_TYPE", "BUILD_TYPE"),
            ("SUBPKG", "SUBPKG"),
            ("BUILD_DIR", "BUILD_DIR"),
            ("STAGING_DIR", "STAGING_DIR")
        ];

        for (lua_name, env_name) in vars {
            if let Ok(val) = globals.get::<String>(lua_name) {
                if cfg!(target_os = "windows") {
                    let _ = writeln!(
                        env_cmds,
                        "$env:{env_name} = '{}'",
                        val.replace('\'', "''")
                    );
                } else {
                    let _ = writeln!(env_cmds, "export {env_name}={val:?}");
                }
            }
        }

        // Handle SYSTEM and ZOI tables
        let tables = [("SYSTEM", "SYSTEM_"), ("ZOI", "ZOI_")];
        for (table_name, prefix) in tables {
            if let Ok(table) = globals.get::<Table>(table_name) {
                for (k, v) in table.pairs::<String, Value>().flatten() {
                    let val_str = match v {
                        Value::String(s) => s
                            .to_str()
                            .map_err(|e| anyhow::anyhow!(e.to_string()))?
                            .to_string(),
                        Value::Integer(i) => i.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Boolean(b) => b.to_string(),
                        _ => continue
                    };
                    let k_upper = k.to_uppercase();
                    if cfg!(target_os = "windows") {
                        let _ = writeln!(
                            env_cmds,
                            "$env:{prefix}{k_upper} = '{}'",
                            val_str.replace('\'', "''")
                        );
                    } else {
                        let _ = writeln!(
                            env_cmds,
                            "export {prefix}{k_upper}={val_str:?}"
                        );
                    }
                }
            }
        }

        stdin.write_all(env_cmds.as_bytes())?;
        stdin.flush()?;

        Ok(Self {
            inner: Mutex::new(InnerSession {
                child,
                stdin,
                stdout,
                stderr_buffer,
                sentinel
            })
        })
    }
}

/// Exposes system command and patching utilities to the Lua environment.
///
/// # Errors
///
/// Returns an error if the `cmd` function cannot be registered in the Lua
/// globals.
///
/// # Panics
///
/// Panics if the internal shell session mutex is poisoned.
pub fn add_cmd_util(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let cmd_fn = lua.create_function(move |lua, command: String| {
        let build_dir: String = lua.globals().get("BUILD_DIR")?;

        let session_is_dead =
            if let Some(session) = lua.app_data_ref::<ShellSession>() {
                let mut inner = session.inner.lock().expect("lock poisoned");
                inner.child.try_wait().map_or(true, |s| s.is_some())
            } else {
                true
            };

        if session_is_dead {
            let session = ShellSession::new(lua, &build_dir)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            lua.set_app_data(session);
        }

        let session = lua
            .app_data_ref::<ShellSession>()
            .expect("ShellSession missing from app_data");
        let mut inner = session.inner.lock().expect("lock poisoned");

        if !quiet {
            println!("Executing: {command}");
        }

        // Clear stderr buffer before running command
        if let Ok(mut buf) = inner.stderr_buffer.lock() {
            buf.clear();
        }

        let sentinel = inner.sentinel.clone();

        if cfg!(target_os = "windows") {
            let cmd_text = format!(
                "$ErrorActionPreference = 'Continue'; & {{ {command} }}; \
                 \"{sentinel} $LASTEXITCODE\"\n"
            );
            inner
                .stdin
                .write_all(cmd_text.as_bytes())
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            inner
                .stdin
                .flush()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        } else {
            let cmd_text = format!(
                "{{ {command} ; }} ; printf \"\\n%s %d\\n\" {sentinel:?} $?\n"
            );
            inner
                .stdin
                .write_all(cmd_text.as_bytes())
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            inner
                .stdin
                .flush()
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        }

        let mut stdout_accum = String::new();
        let mut line = String::new();
        let exit_code;

        loop {
            line.clear();
            let n = inner
                .stdout
                .read_line(&mut line)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            if n == 0 {
                return Err(mlua::Error::RuntimeError(
                    "Shell session ended unexpectedly".to_string()
                ));
            }

            if let Some(idx) = line.find(&sentinel) {
                let out_part = &line[..idx];
                stdout_accum.push_str(out_part);

                let rest = line[idx..]
                    .strip_prefix(&sentinel)
                    .expect("sentinel missing")
                    .trim();
                exit_code = rest.parse::<i32>().unwrap_or(0);
                break;
            }
            stdout_accum.push_str(&line);
        }

        // Retrieve captured stderr
        let stderr = inner
            .stderr_buffer
            .lock()
            .map(|b| b.clone())
            .unwrap_or_default();

        if exit_code != 0 && !quiet {
            eprintln!("[cmd] {stderr}");
        }

        Ok((stdout_accum.trim_end().to_string(), stderr, exit_code))
    })?;
    lua.globals().set("cmd", cmd_fn)?;
    Ok(())
}

/// Adds the `zpatch` function to the Lua environment for applying patches.
///
/// # Errors
///
/// Returns an error if the `zpatch` function cannot be registered in the Lua
/// globals.
pub fn add_zpatch(lua: &Lua, quiet: bool) -> Result<(), mlua::Error> {
    let zpatch_fn = lua.create_function(
        move |lua, (patch_file, strip): (String, Option<u32>)| {
            let build_dir: String = lua.globals().get("BUILD_DIR")?;
            let strip_level = strip.unwrap_or(1);

            if !quiet {
                println!("Applying patch: {patch_file}");
            }

            let output = std::process::Command::new("patch")
                .arg(format!("-p{strip_level}"))
                .arg("-i")
                .arg(&patch_file)
                .current_dir(&build_dir)
                .output();

            match output {
                Ok(out) => {
                    if !out.status.success() {
                        let stderr =
                            String::from_utf8_lossy(&out.stderr).to_string();
                        return Err(mlua::Error::RuntimeError(format!(
                            "patch failed: {stderr}"
                        )));
                    }
                    if !quiet {
                        println!("Successfully applied patch {patch_file}");
                    }
                    Ok(())
                }
                Err(e) => Err(mlua::Error::RuntimeError(format!(
                    "Failed to execute patch command: {e}"
                )))
            }
        }
    )?;
    lua.globals().set("zpatch", zpatch_fn)?;
    Ok(())
}
