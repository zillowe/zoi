pub use zoi_audit as audit;
pub use zoi_core::{
    cache, config, dependency, frozen, hash, lock, offline, pgp, pin, pkgdir,
    recorder, sysroot, types, utils
};
/// Built-in registry lookup and loading.
pub mod builtin_registries {
    pub use zoi_core::builtin::registry::*;
}
pub use zoi_db as db;
pub use zoi_deps as dependencies;
pub use zoi_plugins::extension;
pub use zoi_uninstall::autoremove;
pub mod helper;
/// Package merging utilities.
pub mod merge;
pub use zoi_hooks as hooks;
pub use zoi_install as install;
pub use zoi_lua as lua;
pub use zoi_resolver::local;
/// Package management utilities.
pub mod package;
pub use zoi_deps as pm;
pub use zoi_plugins as plugin;
pub use zoi_purl as purl;
/// Repository installation utilities.
pub mod repo_install;

/// Package creation utilities.
pub mod create {
    pub use zoi_install::create::*;
}
/// Service management utilities.
pub mod service {
    pub use zoi_install::service::*;
}
/// Package registry utilities.
pub mod registry {
    pub use zoi_package::registry::*;
}
/// Package archive delta generation and application.
pub mod delta {
    pub use zoi_package::delta::*;
}
/// System doctor utilities.
pub mod doctor {
    pub use zoi_package::doctor_system::*;
}
/// Package resolution utilities.
pub mod resolve {
    pub use zoi_resolver::resolve::{
        PackageRequest, ResolvedSource, get_db_root, get_default_version,
        parse_source_string, resolve_channel, resolve_package_and_version,
        resolve_requested_version_spec, resolve_source,
        resolve_version_from_url
    };
}
/// Minimal package resolution utilities.
pub mod mini_resolve {
    pub use zoi_core::types::MiniVulnerability;
    pub use zoi_resolver::mini_resolve::{
        MiniPackageIndex, MiniRegistryIndex, check_vulnerabilities,
        fetch_registry_config, fetch_registry_index, get_package_lua_url
    };
}
#[cfg(target_os = "linux")]
pub use zoi_sandbox as sandbox;
pub use zoi_transaction::rollback;
/// Shim management utilities.
pub mod shim {
    pub use zoi_install::shim::*;
}
pub use zoi_core::upgrade;
pub use zoi_sync as sync;
pub use zoi_telemetry as telemetry;
pub use zoi_transaction as transaction;
pub use zoi_uninstall as uninstall;
