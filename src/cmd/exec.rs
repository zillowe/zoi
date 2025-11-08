use crate::pkg;
use anyhow::Result;

pub fn run(
    source: String,
    args: Vec<String>,
    upstream: bool,
    cache: bool,
    local: bool,
) -> Result<i32> {
    pkg::exec::run(&source, args, upstream, cache, local)
}
