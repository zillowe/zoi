use crate::config::SystemConfig;
use crate::generation::Generation;
use serde::{Deserialize, Serialize};

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
