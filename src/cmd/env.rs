use crate::project::{config, environment};
use anyhow::Result;
use colored::*;

pub fn run(env_alias: Option<String>) -> Result<()> {
    let config = config::load()?;
    environment::setup(env_alias.as_deref(), &config)?;
    println!("\n{}", "Environment setup complete.".green());
    Ok(())
}
