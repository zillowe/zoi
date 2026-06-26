use anyhow::Result;
use zoi_project::{config, runner};

pub fn run(cmd_alias: Option<String>, args: Vec<String>) -> Result<()> {
    let config = config::load()?;
    runner::run(cmd_alias.as_deref(), &args, &config)
}
