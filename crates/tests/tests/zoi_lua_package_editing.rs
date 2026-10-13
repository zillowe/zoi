//! Tests for editing the `packages({...})` block of a `zoi.lua`.
//!
//! The block accepts two entry shapes - a plain string and a keyed table with
//! options - and the editor has to understand both. These tests cover that
//! round trip, plus sub-package matching on removal.

use std::fs;

use tempfile::tempdir;

mod common;

/// Writes a `zoi.lua` in a fresh temp dir.
///
/// The returned `TempDir` must be held for the duration of the test: dropping
/// it deletes the directory the test just wrote into.
fn project(
    body: &str
) -> (
    common::TestContextGuard,
    tempfile::TempDir,
    std::path::PathBuf
) {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_current_dir(&root);
    fs::write(root.join("zoi.lua"), body).expect("write zoi.lua");
    (ctx, tmp, root)
}

fn read(root: &std::path::Path) -> String {
    fs::read_to_string(root.join("zoi.lua")).expect("read zoi.lua")
}

const KEYED: &str = "project({ name = \"p\" })\npackages({\n    \
                     [\"@community/fzf\"] = {\n        version = \
                     \"0.44.1\",\n    },\n})\n";

// A keyed entry already declares the package, so adding it again must not
// produce a second entry. In a Lua table the later key wins, so the duplicate
// would silently override whatever the user configured.
#[test]
fn test_add_does_not_duplicate_keyed_entry() {
    let (_ctx, _tmp, root) = project(KEYED);
    zoi::project::config::add_packages_to_config(&[
        "@community/fzf".to_string()
    ])
    .expect("add should succeed");
    let out = read(&root);
    assert_eq!(
        out.matches("@community/fzf").count(),
        1,
        "a keyed entry must not be duplicated:\n{out}"
    );
}

/// The same package requested with an explicit version is still the same
/// package, so it must not be added again.
#[test]
fn test_add_does_not_duplicate_keyed_entry_with_version() {
    let (_ctx, _tmp, root) = project(KEYED);
    zoi::project::config::add_packages_to_config(&[
        "@community/fzf@0.44.1".to_string()
    ])
    .expect("add should succeed");
    let out = read(&root);
    assert_eq!(
        out.matches("@community/fzf").count(),
        1,
        "a versioned spec must still match the keyed entry:\n{out}"
    );
}

// A keyed entry spans several lines, so removal has to drop the whole entry
// rather than just its opening line. Leaving the body behind would produce an
// unparseable zoi.lua.
#[test]
fn test_remove_deletes_whole_keyed_entry() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    [\"@community/fzf\"] = \
         {\n        version = \"0.44.1\",\n    },\n    \"@core/eza\",\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&[
        "@community/fzf".to_string()
    ])
    .expect("remove should succeed");
    let out = read(&root);
    assert!(!out.contains("fzf"), "keyed entry survived:\n{out}");
    assert!(out.contains("@core/eza"), "unrelated entry removed:\n{out}");

    let cfg = zoi::project::config::load().expect("zoi.lua should load");
    assert_eq!(cfg.pkgs, vec!["@core/eza".to_string()]);
}

// `eza` and `eza:icons` are separate installable things. Removing one
// sub-package must leave the base entry and its siblings alone.
#[test]
fn test_remove_sub_package_leaves_base_and_siblings() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    \"@core/eza\",\n    \
         \"@core/eza:icons\",\n    \"@core/eza:docs\",\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&[
        "@core/eza:icons".to_string()
    ])
    .expect("remove should succeed");
    let out = read(&root);
    assert!(!out.contains("eza:icons"), "sub-package survived:\n{out}");
    assert!(out.contains("\"@core/eza\","), "base entry removed:\n{out}");
    assert!(out.contains("eza:docs"), "sibling removed:\n{out}");

    let cfg = zoi::project::config::load().expect("zoi.lua should load");
    let mut pkgs = cfg.pkgs.clone();
    pkgs.sort();
    assert_eq!(
        pkgs,
        vec!["@core/eza".to_string(), "@core/eza:docs".to_string()]
    );
}

// Uninstalling the base package cascades to its sub-packages, matching what
// `zoi uninstall` does on disk.
#[test]
fn test_remove_base_package_cascades_to_sub_packages() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    \"@core/eza\",\n    \
         \"@core/eza:icons\",\n    \"@core/eza:docs\",\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&[
        "@core/eza".to_string()
    ])
    .expect("remove should succeed");
    let out = read(&root);
    assert!(!out.contains("eza"), "cascade did not remove subs:\n{out}");
}

// Removing a plain entry next to a keyed one must leave the keyed entry
// intact and the block loadable.
#[test]
fn test_remove_plain_preserves_keyed_sibling() {
    let (_ctx, _tmp, _root) = project(
        "project({ name = \"p\" })\npackages({\n    [\"@community/fzf\"] = \
         {\n        version = \"0.44.1\",\n    },\n    \"@core/eza\",\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&[
        "@core/eza".to_string()
    ])
    .expect("remove should succeed");
    let cfg = zoi::project::config::load().expect("zoi.lua should load");
    assert_eq!(
        cfg.pkgs,
        vec!["@community/fzf@0.44.1".to_string()],
        "keyed sibling must survive intact"
    );
}

// Removing one entry must not disturb a keyed entry that shares a name prefix.
#[test]
fn test_remove_does_not_clobber_similar_name() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    \
         [\"@community/fzf-extra\"] = {\n        version = \"1.0.0\",\n    \
         },\n    \"@community/fzf\",\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&[
        "@community/fzf".to_string()
    ])
    .expect("remove should succeed");
    let out = read(&root);
    assert!(out.contains("fzf-extra"), "similar name clobbered:\n{out}");
    assert!(
        !out.contains("\"@community/fzf\""),
        "target survived:\n{out}"
    );
}

// Removing one keyed entry must not disturb the next one. Both bodies are
// brace-balanced, so the skip has to track nesting rather than a fixed span.
#[test]
fn test_remove_one_keyed_entry_keeps_sibling() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    [\"@c/a\"] = {\n        \
         version = \"1\",\n    },\n    [\"@c/b\"] = {\n        version = \
         \"2\",\n    },\n})\n"
    );
    zoi::project::config::remove_packages_from_config(&["@c/a".to_string()])
        .expect("remove should succeed");
    let out = read(&root);
    assert!(!out.contains("@c/a"), "target survived:\n{out}");
    assert!(out.contains("@c/b"), "sibling keyed entry removed:\n{out}");
}

/// `--save` records whatever source string the user passed, which is often a
/// local path rather than a registry name. Those entries must still dedupe and
/// remove cleanly, including with a sub-package selector.
#[test]
fn test_path_source_entries_dedupe_and_remove() {
    let (_ctx, _tmp, root) = project(
        "project({ name = \"p\" })\npackages({\n    \"/x/t.pkg.lua\",\n})\n"
    );
    zoi::project::config::add_packages_to_config(&["/x/t.pkg.lua".to_string()])
        .expect("add should succeed");
    let out = read(&root);
    assert_eq!(
        out.matches("t.pkg.lua").count(),
        1,
        "path entry duplicated:\n{out}"
    );

    zoi::project::config::remove_packages_from_config(&[
        "/x/t.pkg.lua".to_string()
    ])
    .expect("remove should succeed");
    assert!(!read(&root).contains("t.pkg.lua"));
}

#[test]
fn test_path_source_sub_entry_removes_selectively() {
    let (_ctx, _tmp, root) =
        project(
            "project({ name = \"p\" })\npackages({\n    \
             \"/x/t.pkg.lua:extra\",\n    \"/x/t.pkg.lua\",\n})\n"
        );
    zoi::project::config::remove_packages_from_config(&[
        "/x/t.pkg.lua:extra".to_string()
    ])
    .expect("remove should succeed");
    let out = read(&root);
    assert!(!out.contains(":extra"), "sub entry survived:\n{out}");
    assert!(
        out.contains("\"/x/t.pkg.lua\","),
        "base entry wrongly removed:\n{out}"
    );
}
