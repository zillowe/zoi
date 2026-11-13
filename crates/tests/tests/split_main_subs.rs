//! Requesting a split package without a sub-package resolves the
//! dependencies of its `main_subs`.

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use zoi::pkg::{install, types};

mod common;

fn write_pkg(dir: &Path, name: &str, extra: &str) {
    fs::write(
        dir.join(format!("{name}.pkg.lua")),
        format!(
            r#"metadata({{
  name = "{name}",
  repo = "core",
  version = "1.0.0",
  description = "{name}",
  maintainer = {{ name = "t", email = "t@example.com" }},
  types = {{ "source" }},
{extra}
}})
"#
        )
    )
    .expect("write pkg.lua");
}

fn resolve(source: &str) -> Vec<String> {
    let (graph, _) = install::resolver::resolve_dependency_graph(
        &[source.to_string()],
        Some(types::Scope::User),
        false,
        true,
        false,
        None,
        true,
        None
    )
    .expect("resolves");
    let mut names: Vec<String> = graph
        .nodes
        .values()
        .map(|n| match &n.sub_package {
            Some(sub) => format!("{}:{sub}", n.pkg.name),
            None => n.pkg.name.clone()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn split_package_without_sub_resolves_main_subs_dependencies() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    ctx.set_env_var("HOME", &root);
    common::TestContextGuard::set_sysroot(root.clone());

    write_pkg(&root, "leafmain", "");
    write_pkg(&root, "leafdev", "");
    write_pkg(
        &root,
        "coll",
        r#"  type = "collection",
  sub_packages = { "main", "dev" },
  main_subs = { "main" },
"#
    );
    let coll = root.join("coll.pkg.lua");
    let mut lua = fs::read_to_string(&coll).expect("read");
    lua.push_str(
        r#"
dependencies({
  runtime = {
    sub_packages = {
      main = { required = { "zoi:./leafmain.pkg.lua" } },
      dev = { required = { "zoi:./leafdev.pkg.lua" } },
    },
  },
})
"#
    );
    fs::write(&coll, lua).expect("write");
    let coll = coll.to_string_lossy().to_string();

    let plain = resolve(&coll);
    assert!(plain.contains(&"leafmain".to_string()), "{plain:?}");
    assert!(!plain.contains(&"leafdev".to_string()), "{plain:?}");

    let dev = resolve(&format!("{coll}:dev"));
    assert!(dev.contains(&"leafdev".to_string()), "{dev:?}");
    assert!(!dev.contains(&"leafmain".to_string()), "{dev:?}");
}
