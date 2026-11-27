//! Sub-packages declaring their own `bins` and `provides`.

use std::fs;

use tempfile::tempdir;
use zoi::pkg::lua::parser;
use zoi::pkg::types::Package;

/// Parses a recipe whose metadata ends with `extra`.
fn parse(extra: &str) -> Package {
    let tmp = tempdir().expect("tempdir");
    let path = tmp.path().join("split.pkg.lua");
    fs::write(
        &path,
        format!(
            r#"metadata({{
    name = "split",
    repo = "core",
    version = "1.0.0",
    description = "split package",
    maintainer = {{ name = "test", email = "test@example.com" }},
    types = {{ "source" }},
    {extra}
}})
"#
        )
    )
    .expect("write pkg.lua");
    parser::parse_lua_package(
        path.to_str().expect("utf-8 path"),
        None,
        None,
        true
    )
    .expect("parse")
}

/// `pkg` narrowed to `sub`, as the installer sees it.
fn for_sub(pkg: &Package, sub: Option<&str>) -> Package {
    let mut narrowed = pkg.clone();
    narrowed.apply_sub_package(sub);
    narrowed
}

const SPLIT: &str = r#"
    sub_packages = { "main", "daemon", "libs" },
    main_subs = { "main" },
    bins = { "split" },
    provides = { "split-tool" },
    sub_bins = { daemon = { "splitd" }, libs = {} },
    sub_provides = { libs = { "libsplit" } },
"#;

#[test]
fn sub_bins_and_sub_provides_are_parsed() {
    let pkg = parse(SPLIT);
    let sub_bins = pkg.sub_bins.as_ref().expect("sub_bins");
    assert_eq!(
        sub_bins.get("daemon").map(Vec::as_slice),
        Some(&["splitd".to_string()][..])
    );
    assert_eq!(sub_bins.get("libs").map(Vec::len), Some(0));
    assert!(pkg.sub_provides.as_ref().is_some_and(|m| m.contains_key("libs")));
}

#[test]
fn a_listed_sub_package_uses_its_own_lists() {
    let pkg = parse(SPLIT);

    let daemon = for_sub(&pkg, Some("daemon"));
    assert_eq!(daemon.bins, Some(vec!["splitd".to_string()]));
    // Not listed in sub_provides: keeps the package-wide provides.
    assert_eq!(daemon.provides, Some(vec!["split-tool".to_string()]));

    let libs = for_sub(&pkg, Some("libs"));
    assert_eq!(libs.bins, Some(Vec::new()), "an empty list links nothing");
    assert_eq!(libs.provides, Some(vec!["libsplit".to_string()]));
}

#[test]
fn unlisted_and_default_sub_packages_use_the_package_lists() {
    let pkg = parse(SPLIT);
    for sub in [Some("main"), None] {
        let narrowed = for_sub(&pkg, sub);
        assert_eq!(narrowed.bins, Some(vec!["split".to_string()]), "{sub:?}");
        assert_eq!(narrowed.provides, Some(vec!["split-tool".to_string()]));
    }
}

#[test]
fn a_bare_request_stands_for_the_first_main_sub_package() {
    let pkg = parse(
        r#"
    sub_packages = { "main", "daemon" },
    main_subs = { "daemon" },
    bins = { "split" },
    sub_bins = { daemon = { "splitd" } },
"#
    );
    assert_eq!(pkg.effective_sub_package(None), Some("daemon"));
    assert_eq!(for_sub(&pkg, None).bins, Some(vec!["splitd".to_string()]));
}

#[test]
fn packages_without_sub_packages_are_unchanged() {
    let pkg = parse(r#"bins = { "plain" },"#);
    assert_eq!(pkg.effective_sub_package(None), None);
    assert_eq!(for_sub(&pkg, None).bins, Some(vec!["plain".to_string()]));
}
