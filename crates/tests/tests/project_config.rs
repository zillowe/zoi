//! Tests for project configuration deserialization and management.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;
use zoi::pkg::repo_install::RepoWorkspace;
use zoi::project::{config, lua_config};

#[test]
fn test_deserialize_project_config_basic() {
    let yaml = r"
name: my-test-project
config:
  local: true
pkgs:
  - eza
  - bat
";
    let cfg: config::ProjectConfig =
        serde_yaml::from_str(yaml).expect("unwrap failed");
    assert_eq!(cfg.name, "my-test-project");
    assert!(cfg.config.local);
    assert_eq!(cfg.pkgs.len(), 2);
    assert!(cfg.pkgs.contains(&"eza".to_string()));
}

#[test]
fn test_deserialize_project_config_versioned_pkgs() {
    let yaml = r#"
name: versioned-project
pkgs:
  - fzf: "0.44.1"
  - fd: "8.7.0"
"#;
    let cfg: config::ProjectConfig =
        serde_yaml::from_str(yaml).expect("unwrap failed");
    assert!(
        cfg.pkgs.contains(&"fzf@0.44.1".to_string())
            || cfg.pkgs.contains(&"fzf: 0.44.1".to_string())
    );
}

#[test]
fn test_load_unified_zoi_lua_declarations() {
    let temp = tempdir().expect("tempdir should be created");
    let path = temp.path().join("zoi.lua");
    fs::write(
        &path,
        r#"
project({ name = "unified" })
imports({
    base = { repo = "zillowe/base", rev = "main" },
})
packages({ "@core/hello" })
package({
    main = "./hello.pkg.lua",
    packages = { extra = "./extra.pkg.lua" },
})
tasks({ { cmd = "build", run = "cargo build" } })
"#
    )
    .expect("zoi.lua should be written");

    let config =
        lua_config::load_zoi_lua(&path, &std::collections::HashMap::new())
            .expect("zoi.lua should load");
    let exports = config
        .package_export
        .expect("package export should be parsed");
    let base_import = config
        .imports
        .get("base")
        .expect("base import should exist");
    let extra_export = exports
        .packages
        .get("extra")
        .expect("extra export should exist");

    assert_eq!(config.name, "unified");
    assert_eq!(base_import.repo, "zillowe/base");
    assert_eq!(base_import.rev.as_deref(), Some("main"));
    assert_eq!(exports.main.as_deref(), Some("./hello.pkg.lua"));
    assert_eq!(extra_export, "./extra.pkg.lua");
}

#[test]
fn test_repo_zoi_lua_accepts_package_only_manifest() {
    let temp = tempdir().expect("tempdir should be created");
    let path = temp.path().join("zoi.lua");
    fs::write(&path, "package('./hello.pkg.lua')\n")
        .expect("zoi.lua should be written");

    let (imports, exports) =
        lua_config::load_repo_zoi_lua(&path).expect("repo zoi.lua should load");
    let exports = exports.expect("package export should exist");

    assert!(imports.is_empty());
    assert_eq!(exports.main.as_deref(), Some("./hello.pkg.lua"));
}

fn initialize_repository(path: &Path) {
    for (args, directory) in [
        (vec!["init"], path),
        (vec!["config", "user.email", "test@example.com"], path),
        (vec!["config", "user.name", "Test"], path),
        (vec!["config", "commit.gpgsign", "false"], path),
        (vec!["add", "."], path),
        (vec!["commit", "-m", "fixture"], path)
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(directory)
            .status()
            .expect("git should run");
        assert!(status.success());
    }
}

#[test]
fn test_repo_workspace_clones_and_selects_package_export() {
    let temp = tempdir().expect("tempdir should be created");
    let repository = temp.path().join("repository");
    std::fs::create_dir_all(repository.join("packages/hello"))
        .expect("package directory should be created");
    fs::write(
        repository.join("zoi.lua"),
        "package({ main = './packages/hello', packages = { extra = \
         './extra.pkg.lua' } })"
    )
    .expect("zoi.lua should be written");
    fs::write(
        repository.join("packages/hello/hello.pkg.lua"),
        "metadata({ name = 'hello', repo = 'community', version = '1.0.0', \
         description = 'Hello', maintainer = { name = 'Test', email = \
         'test@example.com' }, types = { 'source' } })"
    )
    .expect("package definition should be written");
    fs::write(
        repository.join("extra.pkg.lua"),
        "metadata({ name = 'extra', repo = 'community', version = '1.0.0', \
         description = 'Extra', maintainer = { name = 'Test', email = \
         'test@example.com' }, types = { 'source' } })"
    )
    .expect("package definition should be written");
    initialize_repository(&repository);

    let spec = format!("file://{}#extra", repository.display());
    let workspace =
        RepoWorkspace::prepare(&spec).expect("repository should clone");

    let source = workspace
        .sources()
        .first()
        .expect("selected source should exist");
    assert_eq!(workspace.sources().len(), 1);
    assert!(source.ends_with("extra.pkg.lua"));
    assert_eq!(workspace.revision().len(), 40);
}

#[test]
fn test_repo_workspace_resolves_imported_package() {
    let temp = tempdir().expect("tempdir should be created");
    let imported = temp.path().join("imported");
    std::fs::create_dir_all(&imported)
        .expect("import directory should be created");
    fs::write(
        imported.join("zoi.lua"),
        "package({ main = './hello.pkg.lua', packages = { hello = \
         './hello.pkg.lua' } })"
    )
    .expect("imported zoi.lua should be written");
    fs::write(
        imported.join("hello.pkg.lua"),
        "metadata({ name = 'imported-hello', repo = 'community', version = \
         '1.0.0', description = 'Hello', maintainer = { name = 'Test', email \
         = 'test@example.com' }, types = { 'source' } })"
    )
    .expect("imported package should be written");
    initialize_repository(&imported);

    let repository = temp.path().join("repository");
    std::fs::create_dir_all(&repository).expect("repository should be created");
    fs::write(
        repository.join("zoi.lua"),
        format!(
            "imports({{ base = {{ repo = 'file://{}' }} }}) \
             package('base:hello')",
            imported.display()
        )
    )
    .expect("zoi.lua should be written");
    initialize_repository(&repository);

    let workspace =
        RepoWorkspace::prepare(&format!("file://{}", repository.display()))
            .expect("repository and import should clone");

    let source = workspace
        .sources()
        .first()
        .expect("imported source should exist");
    assert!(source.ends_with("hello.pkg.lua"));
    assert!(source.contains("imports/base"));
}

#[test]
fn test_project_imports_materialize_under_zoi_directory() {
    let temp = tempdir().expect("tempdir should be created");
    let imported = temp.path().join("imported");
    std::fs::create_dir_all(&imported)
        .expect("import directory should be created");
    fs::write(
        imported.join("zoi.lua"),
        "package({ main = './hello.pkg.lua', packages = { hello = \
         './hello.pkg.lua' } })"
    )
    .expect("imported zoi.lua should be written");
    fs::write(
        imported.join("hello.pkg.lua"),
        "metadata({ name = 'project-import', repo = 'community', version = \
         '1.0.0', description = 'Hello', maintainer = { name = 'Test', email \
         = 'test@example.com' }, types = { 'source' } })"
    )
    .expect("imported package should be written");
    initialize_repository(&imported);

    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).expect("project should be created");
    let manifest = project.join("zoi.lua");
    fs::write(
        &manifest,
        format!(
            "project({{ name = 'test' }})\nimports({{ base = {{ repo = 'file://{}' }} }})\npackages({{ 'base:hello' }})\n",
            imported.display()
        ),
    )
    .expect("project zoi.lua should be written");

    let mut config =
        lua_config::load_zoi_lua(&manifest, &std::collections::HashMap::new())
            .expect("project config should load");
    let imports = zoi::project::imports::resolve(&mut config, &project)
        .expect("project import should resolve");

    let import = imports.get("base").expect("base import should exist");
    let package = config.pkgs.first().expect("package should exist");
    assert_eq!(import.resolved_revision.len(), 40);
    assert!(package.ends_with("hello.pkg.lua"));
    assert!(package.contains(".zoi/imports/base"));
}

fn write_package_definition(path: &Path, name: &str) {
    fs::write(
        path,
        format!(
            "metadata({{ name = '{name}', repo = 'community', version = \
             '1.0.0', description = 'Hello', maintainer = {{ name = 'Test', \
             email = 'test@example.com' }}, types = {{ 'source' }} }})"
        )
    )
    .expect("package definition should be written");
}

#[test]
fn test_project_imports_resolve_nested_imports() {
    let temp = tempdir().expect("tempdir should be created");
    let leaf = temp.path().join("leaf");
    std::fs::create_dir_all(&leaf).expect("leaf directory should be created");
    fs::write(
        leaf.join("zoi.lua"),
        "package({ main = './hello.pkg.lua', packages = { hello = \
         './hello.pkg.lua' } })"
    )
    .expect("leaf zoi.lua should be written");
    write_package_definition(&leaf.join("hello.pkg.lua"), "nested-hello");
    initialize_repository(&leaf);

    let parent = temp.path().join("parent");
    std::fs::create_dir_all(&parent)
        .expect("parent directory should be created");
    fs::write(
        parent.join("zoi.lua"),
        format!(
            "imports({{ leaf = {{ repo = 'file://{}' }} }})\npackage({{ main \
             = 'leaf:hello' }})",
            leaf.display()
        )
    )
    .expect("parent zoi.lua should be written");
    write_package_definition(&parent.join("unused.pkg.lua"), "parent-unused");
    initialize_repository(&parent);

    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).expect("project should be created");
    let manifest = project.join("zoi.lua");
    fs::write(
        &manifest,
        format!(
            "project({{ name = 'nested' }})\nimports({{ base = {{ repo = \
             'file://{}' }} }})\npackages({{ 'base:main' }})\n",
            parent.display()
        )
    )
    .expect("project zoi.lua should be written");

    let mut config =
        lua_config::load_zoi_lua(&manifest, &std::collections::HashMap::new())
            .expect("project config should load");
    let imports = zoi::project::imports::resolve(&mut config, &project)
        .expect("nested import should resolve");

    let nested = imports
        .get("base/leaf")
        .expect("nested import should be locked");
    assert_eq!(nested.resolved_revision.len(), 40);
    assert!(nested.manifest_hash.starts_with("sha512-"));
    assert!(
        nested
            .exports
            .get("hello")
            .is_some_and(|path| path.ends_with("hello.pkg.lua"))
    );

    let parent_import =
        imports.get("base").expect("parent import should exist");
    let main = parent_import
        .exports
        .get("main")
        .expect("parent main export should exist");
    assert!(main.ends_with("hello.pkg.lua"));
    assert!(main.contains("imports/base/.zoi/imports/leaf"));
    assert!(
        project
            .join(".zoi/imports/base/.zoi/imports/leaf/hello.pkg.lua")
            .is_file()
    );

    let package = config.pkgs.first().expect("package should exist");
    assert!(package.ends_with("hello.pkg.lua"));
    assert!(package.contains("imports/base/.zoi/imports/leaf"));
}

#[test]
fn test_project_imports_reject_cycles() {
    let temp = tempdir().expect("tempdir should be created");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    std::fs::create_dir_all(&first).expect("first directory should be created");
    std::fs::create_dir_all(&second)
        .expect("second directory should be created");
    write_package_definition(&first.join("hello.pkg.lua"), "first-hello");
    write_package_definition(&second.join("hello.pkg.lua"), "second-hello");

    // `first` imports `second` and `second` imports `first`, so resolving the
    // top-level import must detect the cycle before cloning forever.
    fs::write(
        first.join("zoi.lua"),
        format!(
            "imports({{ second = {{ repo = 'file://{}' }} \
             }})\npackage('./hello.pkg.lua')",
            second.display()
        )
    )
    .expect("first zoi.lua should be written");
    initialize_repository(&first);
    fs::write(
        second.join("zoi.lua"),
        format!(
            "imports({{ first = {{ repo = 'file://{}' }} \
             }})\npackage('./hello.pkg.lua')",
            first.display()
        )
    )
    .expect("second zoi.lua should be written");
    initialize_repository(&second);

    let project = temp.path().join("project");
    std::fs::create_dir_all(&project).expect("project should be created");
    let manifest = project.join("zoi.lua");
    fs::write(
        &manifest,
        format!(
            "project({{ name = 'cycle' }})\nimports({{ first = {{ repo = \
             'file://{}' }} }})\n",
            first.display()
        )
    )
    .expect("project zoi.lua should be written");

    let mut config =
        lua_config::load_zoi_lua(&manifest, &std::collections::HashMap::new())
            .expect("project config should load");
    let error = zoi::project::imports::resolve(&mut config, &project)
        .expect_err("cyclic imports should be rejected");

    assert!(
        error.to_string().contains("Import cycle detected"),
        "unexpected error: {error}"
    );
}
