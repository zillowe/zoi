use crate::project::{config, environment};
use anyhow::Result;
use clap_complete::Shell;
use colored::*;

pub fn run(env_alias: Option<String>, export_shell: Option<Shell>) -> Result<()> {
    let config = config::load()?;
    if let Some(shell) = export_shell {
        environment::export_shell(env_alias.as_deref(), &config, shell)?;
    } else {
        environment::setup(env_alias.as_deref(), &config)?;
        println!("\n{}", "Environment setup complete.".green());
    }
    Ok(())
}
