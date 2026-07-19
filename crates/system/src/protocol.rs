use crate::config::SystemConfig;
use crate::generation::Generation;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    ApplySystemConfig(Box<SystemConfig>),
    ListGenerations,
    RollbackGeneration(u32),
    GetStatus,
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Success(String),
    Generations(Vec<Generation>),
    Status(String),
    Error(String),
}

pub fn send_message<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> Result<()> {
    let bytes = serde_json::to_vec(msg)?;
    let len = bytes.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

pub fn receive_message<R: Read, T: for<'a> Deserialize<'a>>(reader: &mut R) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer)?;
    let msg = serde_json::from_slice(&buffer)?;
    Ok(msg)
}
