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

        // `ROOT` is where files are staged during a build (`LOCATION.ROOT`).
        // It is a shell variable, not exported: build systems such as make
        // read the environment and some give `ROOT` a meaning of their own.
        if let Ok(loc) = globals.get::<Table>("LOCATION")
            && let Ok(root) = loc.get::<String>("ROOT")
        {
            if cfg!(target_os = "windows") {
                let quoted = root.replace('\'', "''");
                let _ = writeln!(env_cmds, "$ROOT = '{quoted}'");
            } else {
                let _ = writeln!(env_cmds, "ROOT={}", sh_single_quote(&root));
            }
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

/// Options accepted by `cmd` as its second argument.
#[derive(Default)]
struct CmdOptions {
    /// Overrides the session's quiet setting for this call.
    quiet: Option<bool>,
    /// Runs the script with `set -e` and raises a Lua error when it fails.
    check: bool,
    /// Environment variables set for this call only.
    env: Vec<(String, String)>
}

impl CmdOptions {
    /// Reads `cmd`'s second argument: a boolean (quiet) or an options table
    /// `{ quiet = bool, check = bool, env = { NAME = value } }`.
    fn parse(opts: Option<&Value>) -> mlua::Result<Self> {
        let table = match opts {
            None | Some(Value::Nil) => return Ok(Self::default()),
            Some(Value::Boolean(quiet)) => {
                return Ok(Self {
                    quiet: Some(*quiet),
                    ..Self::default()
                });
            }
            Some(Value::Table(table)) => table,
            Some(other) => {
                return Err(mlua::Error::RuntimeError(format!(
                    "cmd: options must be a boolean or a table, got {}",
                    other.type_name()
                )));
            }
        };
        let mut env = Vec::new();
        if let Some(vars) = table.get::<Option<Table>>("env")? {
            for pair in vars.pairs::<String, Value>() {
                let (name, value) = pair?;
                if !is_valid_env_name(&name) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "cmd: invalid environment variable name '{name}'"
                    )));
                }
                let value = match value {
                    Value::String(s) => s.to_str()?.to_string(),
                    Value::Integer(i) => i.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Boolean(b) => b.to_string(),
                    other => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "cmd: env value for '{name}' must be a string, \
                             number or boolean, got {}",
                            other.type_name()
                        )));
                    }
                };
                env.push((name, value));
            }
        }
        // Stable order, so the generated script does not depend on Lua's
        // table iteration order.
        env.sort();
        Ok(Self {
            quiet: table.get::<Option<bool>>("quiet")?,
            check: table.get::<Option<bool>>("check")?.unwrap_or(false),
            env
        })
    }

    /// The script sent to a POSIX shell. With `check` or `env` it runs in a
    /// subshell, so `set -e`, the variables and any `cd` stay inside it and
    /// a failure cannot end the session.
    fn posix_script(&self, command: &str) -> String {
        if !self.check && self.env.is_empty() {
            return command.to_string();
        }
        let mut script = String::from("(\n");
        if self.check {
            script.push_str("set -e\n");
        }
        for (name, value) in &self.env {
            push_env_assignment(&mut script, name, value);
        }
        script.push_str(command);
        script.push_str("\n)");
        script
    }

    /// The statement sent to the `PowerShell` session. It prints the
    /// sentinel followed by the exit code.
    fn powershell_script(&self, command: &str, sentinel: &str) -> String {
        let mut body = String::new();
        if self.check {
            body.push_str(
                "$ErrorActionPreference = 'Stop'; \
                 $PSNativeCommandUseErrorActionPreference = $true\n"
            );
        }
        if self.env.is_empty() {
            body.push_str(command);
        } else {
            // `$env:` is process-wide, so the previous values are restored.
            let names = self
                .env
                .iter()
                .map(|(name, _)| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(",");
            let _ = write!(
                body,
                "$__zoiSaved = @{{}}; foreach ($__k in @({names})) {{ \
                 $__zoiSaved[$__k] = \
                 [Environment]::GetEnvironmentVariable($__k) }}\ntry {{\n"
            );
            for (name, value) in &self.env {
                push_env_assignment(&mut body, name, value);
            }
            let _ = write!(
                body,
                "{command}\n}} finally {{ foreach ($__k in $__zoiSaved.Keys) \
                 {{ [Environment]::SetEnvironmentVariable($__k, \
                 $__zoiSaved[$__k]) }} }}"
            );
        }
        if self.check {
            format!(
                "$__zoiFailed = $false; try {{ & {{ {body} }} }} catch {{ \
                 $__zoiFailed = $true; [Console]::Error.WriteLine($_) }}; \
                 $__zoiCode = if ($__zoiFailed -and -not $LASTEXITCODE) {{ 1 \
                 }} else {{ $LASTEXITCODE }}; \"{sentinel} $__zoiCode\"\n"
            )
        } else {
            format!(
                "$ErrorActionPreference = 'Continue'; & {{ {body} }}; \
                 \"{sentinel} $LASTEXITCODE\"\n"
            )
        }
    }
}

/// Exposes system command and patching utilities to the Lua environment.
///
/// `cmd(script, opts)` runs `script` in a shell session that persists for the
/// whole Lua environment, so a plain `cd` carries over to the next call.
/// `opts` is a boolean (quiet) or a table with `quiet`, `check` (`set -e` and
/// a Lua error on failure) and `env` (variables for this call only).
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
            let options = CmdOptions::parse(opts.as_ref())?;

            // A command such as `exit` ends the session. The next call then
            // starts a fresh one; the command itself is never run twice.
            let session_is_dead =
                lua.app_data_ref::<ShellSession>().is_none_or(|session| {
                    let mut inner =
                        session.inner.lock().expect("lock poisoned");
                    inner.child.try_wait().map_or(true, |s| s.is_some())
                });
            if session_is_dead {
                let quiet_flag =
                    lua.app_data_ref::<ShellSessionConfig>().map_or_else(
                        || Arc::new(AtomicBool::new(quiet)),
                        |c| Arc::clone(&c.quiet)
                    );
                let session = ShellSession::new(lua, &build_dir, quiet_flag)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                lua.set_app_data(session);
            }

            let session = lua
                .app_data_ref::<ShellSession>()
                .expect("ShellSession missing from app_data");
            let cmd_quiet = options
                .quiet
                .unwrap_or_else(|| session.quiet.load(Ordering::Relaxed));

            let mut inner = session.inner.lock().expect("lock poisoned");

            if !cmd_quiet {
                println!("Executing: {command}");
            }

            // Clear stderr buffer before running command
            if let Ok(mut buf) = inner.stderr_buffer.lock() {
                buf.clear();
            }

            let sentinel = inner.sentinel.clone();
            let cmd_text = if cfg!(target_os = "windows") {
                options.powershell_script(&command, &sentinel)
            } else {
                format!(
                    "{{\n{}\n}}\nprintf \"\\n%s %d\\n\" {sentinel:?} $?\n",
                    options.posix_script(&command)
                )
            };
            inner
                .stdin
                .write_all(cmd_text.as_bytes())
                .and_then(|()| inner.stdin.flush())
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

            let mut stdout_accum = String::new();
            let mut line = String::new();
            let mut exit_code = None;
            loop {
                line.clear();
                let n = inner
                    .stdout
                    .read_line(&mut line)
                    .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                if n == 0 {
                    break;
                }

                if let Some((out_part, rest)) = line.split_once(&sentinel) {
                    stdout_accum.push_str(out_part);
                    if !cmd_quiet && !out_part.is_empty() {
                        print!("{out_part}");
                        let _ = std::io::stdout().flush();
                    }
                    exit_code = Some(rest.trim().parse::<i32>().unwrap_or(0));
                    break;
                }
                stdout_accum.push_str(&line);
                if !cmd_quiet {
                    print!("{line}");
                    let _ = std::io::stdout().flush();
                }
            }

            // No sentinel: the command ended the shell. Its exit status is
            // the command's result.
            let exit_code = match exit_code {
                Some(code) => code,
                None => inner
                    .child
                    .wait()
                    .ok()
                    .and_then(|status| status.code())
                    .unwrap_or(1)
            };

            // Retrieve captured stderr
            let stderr = inner
                .stderr_buffer
                .lock()
                .map(|b| b.clone())
                .unwrap_or_default();

            if exit_code != 0 {
                if options.check {
                    return Err(mlua::Error::RuntimeError(format!(
                        "command failed with exit code {exit_code}\n{}",
                        stderr.trim_end()
                    )));
                }
                if !cmd_quiet {
                    eprintln!(
                        "[cmd] Command failed with exit code: {exit_code}"
                    );
                }
            }

            Ok((stdout_accum.trim_end().to_string(), stderr, exit_code))
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
