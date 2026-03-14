use std::fs;
use tempfile::tempdir;
use zoi::pkg::{extension, plugin, sysroot};

#[test]
fn test_extension_add_reverts_cleanly() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    unsafe {
        std::env::set_var("HOME", root.clone());
    }
    sysroot::set_sysroot(root.clone());

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
