use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use mlua::{self, Lua, Table, Value};

/// Quotes a value for a POSIX shell using single quotes.
///
/// Inside single quotes nothing is expanded (`$`, backticks, `\`), so the
/// only character needing care is `'` itself, which is written as `'\''`.
pub(crate) fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Reports whether `name` is safe to use as an environment variable name in a
/// generated `export`/`$env:` statement.
fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Appends a statement that sets environment variable `name` to `value` in the
/// session shell, quoting the value so it is never expanded.
fn push_env_assignment(env_cmds: &mut String, name: &str, value: &str) {
    if !is_valid_env_name(name) {
        return;
    }
    if cfg!(target_os = "windows") {
        let _ =
            writeln!(env_cmds, "$env:{name} = '{}'", value.replace('\'', "''"));
    } else {
        let _ = writeln!(env_cmds, "export {name}={}", sh_single_quote(value));
    }
}

/// Configuration for the Lua shell session.
pub struct ShellSessionConfig {
    /// Whether command output should be silenced.
    pub quiet: Arc<AtomicBool>
}

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
    inner: Mutex<InnerSession>,
    /// Whether command output should be silenced.
    quiet: Arc<AtomicBool>
}

impl ShellSession {
    /// Creates a new `ShellSession` within the specified build directory.
    fn new(
        lua: &Lua,
        build_dir: &str,
        quiet: Arc<AtomicBool>
    ) -> Result<Self, anyhow::Error> {
        let sentinel =
            format!("---ZOI_CMD_COMPLETE_{}---", uuid::Uuid::new_v4());

        let mut child = if cfg!(target_os = "windows") {
            Command::new("pwsh")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg("-")
                .env_remove("GPG_PASSWORD")
                .current_dir(build_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        } else {
            Command::new("bash")
                .arg("--noprofile")
                .arg("--norc")
                // The signing passphrase must never reach package code.
                .env_remove("GPG_PASSWORD")
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
        let quiet_clone = Arc::clone(&quiet);

        // Spawn a thread to consume stderr continuously
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if !quiet_clone.load(Ordering::Relaxed) {
                    eprint!("{line}");
                    let _ = std::io::stderr().flush();
                }
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
            ("PKGLUADIR", "PKGLUADIR")
        ];

        for (lua_name, env_name) in vars {
            if let Ok(val) = globals.get::<String>(lua_name) {
                push_env_assignment(&mut env_cmds, env_name, &val);
            }
        }

        // BUILD_DIR and STAGING_DIR come from the Rust-side environment state,
        // not from (script-writable) Lua globals.
        push_env_assignment(&mut env_cmds, "BUILD_DIR", build_dir);
        if let Some(staging_dir) = super::opt_staging_dir(lua) {
            push_env_assignment(&mut env_cmds, "STAGING_DIR", &staging_dir);
        }

        // Also resolve PKGLUADIR from LOCATION table if not set in globals
        if let Ok(loc) = globals.get::<Table>("LOCATION")
            && let Ok(pkg_dir) = loc.get::<String>("PKGLUADIR")
        {
            push_env_assignment(&mut env_cmds, "PKGLUADIR", &pkg_dir);
        }

        // Allow GNU autotools builds to run as root in containerized CI
        // environments
        if cfg!(target_os = "windows") {
            let _ = writeln!(env_cmds, "$env:FORCE_UNSAFE_CONFIGURE = '1'");
        } else {
            let _ = writeln!(env_cmds, "export FORCE_UNSAFE_CONFIGURE='1'");
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
                    push_env_assignment(
                        &mut env_cmds,
                        &format!("{prefix}{k_upper}"),
                        &val_str
                    );
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
            }),
            quiet
        })
    }
}

/// Sets whether the shell session in the Lua environment should run quietly.
pub fn set_session_quiet(lua: &Lua, quiet: bool) {
    if let Some(config) = lua.app_data_ref::<ShellSessionConfig>() {
        config.quiet.store(quiet, Ordering::Relaxed);
    }
    if let Some(session) = lua.app_data_ref::<ShellSession>() {
        session.quiet.store(quiet, Ordering::Relaxed);
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
    let session_config = ShellSessionConfig {
        quiet: Arc::new(AtomicBool::new(quiet))
    };
    let _ = lua.set_app_data(session_config);

    let cmd_fn = lua.create_function(
        move |lua, (command, opts): (String, Option<Value>)| {
            let build_dir = super::build_dir(lua)?;

            // A command such as `exit` can terminate the shell mid-call. Retry
            // the command once against a freshly spawned session so `cmd`
            // transparently recovers instead of failing on the next invocation.
            let mut retried = false;

            let (stdout_accum, stderr, exit_code) = loop {
                let session_is_dead =
                    if let Some(session) = lua.app_data_ref::<ShellSession>() {
                        let mut inner =
                            session.inner.lock().expect("lock poisoned");
                        inner.child.try_wait().map_or(true, |s| s.is_some())
                    } else {
                        true
                    };

                if retried || session_is_dead {
                    let quiet_flag =
                        lua.app_data_ref::<ShellSessionConfig>().map_or_else(
                            || Arc::new(AtomicBool::new(quiet)),
                            |c| Arc::clone(&c.quiet)
                        );
                    let session =
                        ShellSession::new(lua, &build_dir, quiet_flag)
                            .map_err(|e| {
                                mlua::Error::RuntimeError(e.to_string())
                            })?;
                    lua.set_app_data(session);
                }

                let session = lua
                    .app_data_ref::<ShellSession>()
                    .expect("ShellSession missing from app_data");

                let session_quiet = session.quiet.load(Ordering::Relaxed);
                let cmd_quiet = match &opts {
                    Some(Value::Boolean(b)) => *b,
                    Some(Value::Table(t)) => t
                        .get::<Option<bool>>("quiet")
                        .ok()
                        .flatten()
                        .unwrap_or(session_quiet),
                    _ => session_quiet
                };

                let mut inner = session.inner.lock().expect("lock poisoned");

                if !cmd_quiet {
                    println!("Executing: {command}");
                }

                // Clear stderr buffer before running command
                if let Ok(mut buf) = inner.stderr_buffer.lock() {
                    buf.clear();
                }

                let sentinel = inner.sentinel.clone();

                if cfg!(target_os = "windows") {
                    let cmd_text = format!(
                        "$ErrorActionPreference = 'Continue'; & {{ {command} \
                         }}; \"{sentinel} $LASTEXITCODE\"\n"
                    );
                    inner.stdin.write_all(cmd_text.as_bytes()).map_err(
                        |e| mlua::Error::RuntimeError(e.to_string())
                    )?;
                    inner.stdin.flush().map_err(|e| {
                        mlua::Error::RuntimeError(e.to_string())
                    })?;
                } else {
                    let cmd_text = format!(
                        "{{\n{command}\n}}\nprintf \"\\n%s %d\\n\" \
                         {sentinel:?} $?\n"
                    );
                    inner.stdin.write_all(cmd_text.as_bytes()).map_err(
                        |e| mlua::Error::RuntimeError(e.to_string())
                    )?;
                    inner.stdin.flush().map_err(|e| {
                        mlua::Error::RuntimeError(e.to_string())
                    })?;
                }

                let mut stdout_accum = String::new();
                let mut line = String::new();
                let mut exit_code = 0;

                let mut session_died = false;
                loop {
                    line.clear();
                    let n = inner.stdout.read_line(&mut line).map_err(|e| {
                        mlua::Error::RuntimeError(e.to_string())
                    })?;
                    if n == 0 {
                        session_died = true;
                        break;
                    }

                    if let Some(idx) = line.find(&sentinel) {
                        let out_part = &line[..idx];
                        stdout_accum.push_str(out_part);
                        if !cmd_quiet && !out_part.is_empty() {
                            print!("{out_part}");
                            let _ = std::io::stdout().flush();
                        }

                        let rest = line[idx..]
                            .strip_prefix(&sentinel)
                            .expect("sentinel missing")
                            .trim();
                        exit_code = rest.parse::<i32>().unwrap_or(0);
                        break;
                    }
                    stdout_accum.push_str(&line);
                    if !cmd_quiet {
                        print!("{line}");
                        let _ = std::io::stdout().flush();
                    }
                }

                if session_died && !retried {
                    retried = true;
                    continue;
                }
                if session_died {
                    let stderr = inner
                        .stderr_buffer
                        .lock()
                        .map(|b| b.clone())
                        .unwrap_or_default();
                    let err_text = if !stderr.trim().is_empty() {
                        stderr.trim().to_string()
                    } else if !stdout_accum.trim().is_empty() {
                        stdout_accum.trim().to_string()
                    } else {
                        String::new()
                    };
                    return Err(mlua::Error::RuntimeError(
                        if err_text.is_empty() {
                            "Shell session ended unexpectedly".to_string()
                        } else {
                            format!(
                                "Shell session ended unexpectedly:\n{err_text}"
                            )
                        }
                    ));
                }

                // Retrieve captured stderr
                let stderr = inner
                    .stderr_buffer
                    .lock()
                    .map(|b| b.clone())
                    .unwrap_or_default();

                if exit_code != 0 && !cmd_quiet {
                    eprintln!(
                        "[cmd] Command failed with exit code: {exit_code}"
                    );
                }

                break (stdout_accum.trim_end().to_string(), stderr, exit_code);
            };

            Ok((stdout_accum, stderr, exit_code))
        }
    )?;
    lua.globals().set("cmd", cmd_fn)?;

    let set_quiet_fn = lua.create_function(|lua, quiet: bool| {
        set_session_quiet(lua, quiet);
        Ok(())
    })?;
    lua.globals().set("set_quiet", set_quiet_fn)?;

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
            let build_dir = super::build_dir(lua)?;
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
