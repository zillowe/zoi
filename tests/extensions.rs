use std::fs;
use tempfile::tempdir;
use zoi::pkg::{config, extension, local, plugin, types};

mod common;

#[test]
fn test_extension_add_reverts_cleanly() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pm = plugin::PluginManager::new().unwrap();

    let pkg_lua_content = r#"
metadata({
    name = "test-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Test extension",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "plugin", name = "my-plugin", script = "print('hello')" },
            { type = "hook", name = "my-hook", content = "name: my-hook\ntrigger:\n  paths: ['*']\naction:\n  when: PostTransaction\n  exec: echo" }
        }
    }
})
"#;
    let pkg_lua_path = root.join("test-ext.pkg.lua");
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    extension::add(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();

    let plugin_path = root.join(".zoi/plugins/my-plugin.lua");
    let hook_path = root.join(".zoi/hooks/my-hook.hook.yaml");

    assert!(plugin_path.exists(), "Plugin file should be created");
    assert!(hook_path.exists(), "Hook file should be created");

    extension::remove(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();

    assert!(!plugin_path.exists(), "Plugin file should be removed");
    assert!(!hook_path.exists(), "Hook file should be removed");
}

#[test]
fn test_extension_remove_restores_previous_default_registry() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    config::write_user_config(&types::Config {
        default_registry: Some(types::Registry {
            handle: String::new(),
            url: "https://example.com/original.git".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        ..Default::default()
    })
    .unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    let pkg_lua_content = r#"
metadata({
    name = "registry-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Registry extension",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "registry-repo", add = "https://example.com/override.git" }
        }
    }
})
"#;
    let pkg_lua_path = root.join("registry-ext.pkg.lua");
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    extension::add(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    let active_registry = config::read_user_config()
        .unwrap()
        .default_registry
        .unwrap()
        .url;
    assert_eq!(active_registry, "https://example.com/override.git");

    extension::remove(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    let restored_registry = config::read_user_config()
        .unwrap()
        .default_registry
        .unwrap()
        .url;
    assert_eq!(restored_registry, "https://example.com/original.git");
}

#[test]
fn test_extension_add_failure_restores_previous_default_registry() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());
    ctx.set_current_dir(&root);

    config::write_user_config(&types::Config {
        default_registry: Some(types::Registry {
            handle: String::new(),
            url: "https://example.com/original.git".to_string(),
            advisory_prefix: None,
            authorities: None,
        }),
        ..Default::default()
    })
    .unwrap();

    fs::write(root.join("zoi.yaml"), "name: existing-project\n").unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    let pkg_lua_content = r#"
metadata({
    name = "broken-registry-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Registry extension with later failure",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "registry-repo", add = "https://example.com/override.git" },
            { type = "project", add = "name: should-not-overwrite\n" }
        }
    }
})
"#;
    let pkg_lua_path = root.join("broken-registry-ext.pkg.lua");
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    let result = extension::add(pkg_lua_path.to_str().unwrap(), true, &pm);
    assert!(
        result.is_err(),
        "extension add should fail after the registry change"
    );

    let restored_registry = config::read_user_config()
        .unwrap()
        .default_registry
        .unwrap()
        .url;
    assert_eq!(restored_registry, "https://example.com/original.git");
    assert!(
        local::is_package_installed("broken-registry-ext", None, types::Scope::User)
            .unwrap()
            .is_none(),
        "failed extension add should not leave an installed manifest behind"
    );
}

#[test]
fn test_extension_add_failure_rolls_back_created_plugin() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());
    ctx.set_current_dir(&root);

    fs::write(root.join("zoi.yaml"), "name: existing-project\n").unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    let pkg_lua_content = r#"
metadata({
    name = "broken-plugin-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Extension with rollback test",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "plugin", name = "rolled-back-plugin", script = "print('hello')" },
            { type = "project", add = "name: should-not-overwrite\n" }
        }
    }
})
"#;
    let pkg_lua_path = root.join("broken-plugin-ext.pkg.lua");
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    let result = extension::add(pkg_lua_path.to_str().unwrap(), true, &pm);
    assert!(
        result.is_err(),
        "extension add should fail after creating the plugin"
    );
    assert!(
        !root.join(".zoi/plugins/rolled-back-plugin.lua").exists(),
        "failed extension add should roll back created plugin files"
    );
}

#[test]
fn test_extension_remove_project_uses_original_install_directory() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let install_dir = root.join("install-dir");
    let other_dir = root.join("other-dir");

    fs::create_dir_all(&install_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pm = plugin::PluginManager::new().unwrap();
    let pkg_lua_content = r#"
metadata({
    name = "project-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Project extension",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "project", add = "name: generated-project\n" }
        }
    }
})
"#;
    let pkg_lua_path = root.join("project-ext.pkg.lua");
    fs::write(&pkg_lua_path, pkg_lua_content).unwrap();

    ctx.set_current_dir(&install_dir);
    extension::add(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    assert!(install_dir.join("zoi.yaml").exists());

    fs::write(other_dir.join("zoi.yaml"), "name: keep-me\n").unwrap();
    ctx.set_current_dir(&other_dir);

    extension::remove(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();

    assert!(
        !install_dir.join("zoi.yaml").exists(),
        "remove should delete the project file from the original install directory"
    );
    assert!(
        other_dir.join("zoi.yaml").exists(),
        "remove should not touch an unrelated zoi.yaml in the current directory"
    );
}

#[test]
fn test_extension_remove_uses_installed_extension_metadata() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pm = plugin::PluginManager::new().unwrap();
    let pkg_lua_path = root.join("drifting-ext.pkg.lua");
    fs::write(
        &pkg_lua_path,
        r#"
metadata({
    name = "drifting-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Extension drift test",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "plugin", name = "original-plugin", script = "print('original')" }
        }
    }
})
"#,
    )
    .unwrap();

    extension::add(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    assert!(root.join(".zoi/plugins/original-plugin.lua").exists());

    fs::write(
        &pkg_lua_path,
        r#"
metadata({
    name = "drifting-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Extension drift test",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "plugin", name = "different-plugin", script = "print('different')" }
        }
    }
})
"#,
    )
    .unwrap();

    extension::remove(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    assert!(
        !root.join(".zoi/plugins/original-plugin.lua").exists(),
        "remove should revert the installed extension metadata, not the current source metadata"
    );
}

#[test]
fn test_extension_post_hooks_are_nonfatal() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());
    ctx.set_sysroot(root.clone());

    let pm = plugin::PluginManager::new().unwrap();
    pm.lua
        .load(
            r#"
        zoi.on_post_extension_add(function()
            error("post add failure")
        end)
        zoi.on_post_extension_remove(function()
            error("post remove failure")
        end)
    "#,
        )
        .exec()
        .unwrap();

    let pkg_lua_path = root.join("hook-ext.pkg.lua");
    fs::write(
        &pkg_lua_path,
        r#"
metadata({
    name = "hook-ext",
    repo = "community",
    type = "extension",
    version = "1.0",
    description = "Post hook test",
    maintainer = { name = "test", email = "test@test.com" },
    extension = {
        type = "zoi",
        changes = {
            { type = "plugin", name = "hook-plugin", script = "print('hook')" }
        }
    }
})
"#,
    )
    .unwrap();

    extension::add(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    assert!(root.join(".zoi/plugins/hook-plugin.lua").exists());

    extension::remove(pkg_lua_path.to_str().unwrap(), true, &pm).unwrap();
    assert!(!root.join(".zoi/plugins/hook-plugin.lua").exists());
}
