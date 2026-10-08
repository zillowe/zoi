use std::io::{Read, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::SystemConfig;
use crate::generation::Generation;

/// Directory the daemon's runtime state lives in.
///
/// Declared as the unit's `RuntimeDirectory=`, so systemd creates it with the
/// right ownership on start and removes it on stop. Keeping the socket inside
/// this directory rather than directly in `/run` is what guarantees no stale
/// socket survives an unclean shutdown.
pub const RUNTIME_DIR: &str = "/run/zoid";

/// Unix socket the CLI and the `ZoiOS` client connect to.
pub const SOCKET_PATH: &str = "/run/zoid/zoid.sock";

/// File holding the daemon's process id.
pub const PID_PATH: &str = "/run/zoid/zoid.pid";

#[derive(Debug, Serialize, Deserialize)]
/// A request from the CLI to the daemon.
///
/// The socket is trusted: it is mode 0600 inside a root-owned runtime
/// directory, so the daemon treats every request as coming from an
/// administrator. There is no per-request authorization, because the CLI only
/// reaches this socket after the user has already authenticated.
pub enum Request {
    /// Reconcile the machine with a parsed `system.lua`.
    ApplySystemConfig(Box<SystemConfig>),
    /// List the recorded system generations.
    ListGenerations,
    /// Roll the system back to a generation older than the active one.
    RollbackGeneration(u32),
    /// Pin or unpin a generation so it survives pruning.
    PinGeneration(u32, bool),
    /// Report the daemon's current generation.
    GetStatus,
    /// Ask the daemon to exit.
    Shutdown
}

#[derive(Debug, Serialize, Deserialize)]
/// The daemon's reply to a [`Request`].
pub enum Response {
    /// The request succeeded and produced no detail.
    Ok,
    /// The request succeeded, with a human readable summary.
    Success(String),
    /// The recorded generations, oldest first.
    Generations(Vec<Generation>),
    /// A status line for the caller to display.
    Status(String),
    /// The request failed, with the reason.
    Error(String)
}

/// Writes a length-prefixed, JSON-encoded message.
///
/// # Errors
///
/// Returns an error if the message cannot be encoded or written to the
/// stream.
pub fn send_message<W: Write, T: Serialize>(
    writer: &mut W,
    msg: &T
) -> Result<()> {
    let bytes = serde_json::to_vec(msg)?;
    let len = bytes.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Reads one length-prefixed, JSON-encoded message.
///
/// # Errors
///
/// Returns an error if the reply cannot be read, or cannot be decoded
/// into `T`.
pub fn receive_message<R: Read, T: for<'a> Deserialize<'a>>(
    reader: &mut R
) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer)?;
    let msg = serde_json::from_slice(&buffer)?;
    Ok(msg)
}
