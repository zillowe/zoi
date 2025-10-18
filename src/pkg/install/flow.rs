use crate::pkg::install::{installer, resolver};
use crate::pkg::{resolve, types};
use anyhow::Result;
use indicatif::MultiProgress;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum InstallMode {
    PreferPrebuilt,
    ForceBuild,
}

pub fn run_installation(
    source: &str,
    mode: InstallMode,
    _force: bool,
    reason: types::InstallReason,
    _yes: bool,
    _all_optional: bool,
    _processed_deps: &std::sync::Mutex<std::collections::HashSet<String>>,
    scope_override: Option<types::Scope>,
    m: Option<&MultiProgress>,
) -> Result<()> {
    let (mut pkg, version, _, _, registry_handle) = resolve::resolve_package_and_version(source)?;

    if let Some(scope) = scope_override {
        pkg.scope = scope;
    }

    let node = resolver::InstallNode {
        pkg,
        version,
        reason,
        source: source.to_string(),
        registry_handle: registry_handle.unwrap_or_else(|| "local".to_string()),
        chosen_options: vec![],
        chosen_optionals: vec![],
    };

    installer::install_node(&node, mode, m)
}
