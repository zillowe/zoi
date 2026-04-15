use std::fs;
use tempfile::tempdir;
use zoi::pkg::plugin;

mod common;

#[test]
fn test_plugin_on_project_install_hook() {
    let ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    ctx.set_sysroot(root.clone());

    let pm = plugin::PluginManager::new().expect("Failed to create PluginManager");

    pm.lua
        .load(
            r#"
        zoi.on_project_install(function()
            return true
        end)
    "#,
        )
        .exec()
        .expect("Failed to load lua");

    let handled = pm.trigger_project_install_hook().expect("Hook failed");
    assert!(handled, "Hook should have returned true");
}

#[test]
fn test_plugin_fs_symlink_api() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let target = root.join("target.txt");
    let link = root.join("link.txt");
    fs::write(&target, "hello").unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    let script = format!(
        r#"return zoi.fs.symlink("{}", "{}", false)"#,
        target.to_str().unwrap().replace("\\", "/"),
        link.to_str().unwrap().replace("\\", "/")
    );

    let success: bool = pm.lua.load(&script).eval().unwrap();
    assert!(success, "Symlink API should return true");
    assert!(link.exists(), "Link should exist");
    assert_eq!(fs::read_to_string(link).unwrap(), "hello");
}

#[test]
fn test_plugin_archive_extract_api() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let tar_path = root.join("test.tar.gz");
    let source_dir = root.join("src");
    fs::create_dir_all(source_dir.join("package")).unwrap();
    fs::write(source_dir.join("package/hello.txt"), "world").unwrap();

    {
        let file = fs::File::create(&tar_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all("package", source_dir.join("package"))
            .unwrap();
        tar.finish().unwrap();
    }

    let pm = plugin::PluginManager::new().unwrap();
    let dest = root.join("dest");

    let script = format!(
        r#"return zoi.archive.extract("{}", "{}", 1)"#,
        tar_path.to_str().unwrap().replace("\\", "/"),
        dest.to_str().unwrap().replace("\\", "/")
    );

    let success: bool = pm.lua.load(&script).eval().unwrap();
    assert!(success, "Extract API should return true");
    assert!(
        dest.join("hello.txt").exists(),
        "hello.txt should be extracted to dest root due to strip=1"
    );
    assert_eq!(fs::read_to_string(dest.join("hello.txt")).unwrap(), "world");
}

#[test]
fn test_plugin_archive_extract_rejects_path_traversal_with_strip() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let zip_path = root.join("evil.zip");
    {
        use std::io::Write;

        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "package/../../escape.txt",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"escape").unwrap();
        zip.finish().unwrap();
    }

    let pm = plugin::PluginManager::new().unwrap();
    let dest = root.join("dest");
    let escaped = root.join("escape.txt");

    let script = format!(
        r#"return zoi.archive.extract("{}", "{}", 1)"#,
        zip_path.to_str().unwrap().replace("\\", "/"),
        dest.to_str().unwrap().replace("\\", "/")
    );

    let result: Result<bool, mlua::Error> = pm.lua.load(&script).eval();
    assert!(
        result.is_err(),
        "strip extraction should reject path traversal"
    );
    assert!(
        !escaped.exists(),
        "archive extraction should not create files outside the destination"
    );
}

#[test]
fn test_plugin_fs_copy_api() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    let src = root.join("src_dir");
    let dest = root.join("dest_dir");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("file.txt"), "copy-content").unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    let script = format!(
        r#"return zoi.fs.copy("{}", "{}")"#,
        src.to_str().unwrap().replace("\\", "/"),
        dest.to_str().unwrap().replace("\\", "/")
    );

    let success: bool = pm.lua.load(&script).eval().unwrap();
    assert!(success, "Copy API should return true");
    assert!(dest.join("file.txt").exists());
    assert_eq!(
        fs::read_to_string(dest.join("file.txt")).unwrap(),
        "copy-content"
    );
}

#[test]
fn test_plugin_sh_api() {
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();
    let out_file = root.join("sh_out.txt");

    let pm = plugin::PluginManager::new().unwrap();
    let script = format!(
        r#"return zoi.sh("echo 'sh-works' > {}")"#,
        out_file.to_str().unwrap().replace("\\", "/")
    );

    let exit_code: i32 = pm.lua.load(&script).eval().unwrap();
    assert_eq!(exit_code, 0, "zoi.sh should return 0 exit code");
    assert!(out_file.exists());
    assert_eq!(fs::read_to_string(out_file).unwrap().trim(), "sh-works");
}

#[test]
fn test_plugin_env_set_is_session_local() {
    let pm = plugin::PluginManager::new().unwrap();
    let value: Option<String> = pm
        .lua
        .load(
            r#"
        zoi.env.set("ZOI_PLUGIN_TEST_ENV", "session-value")
        return zoi.env.get("ZOI_PLUGIN_TEST_ENV")
    "#,
        )
        .eval()
        .unwrap();
    assert_eq!(value.as_deref(), Some("session-value"));
}

#[test]
fn test_plugin_set_data_rejects_non_finite_number() {
    let pm = plugin::PluginManager::new().unwrap();
    let result: Result<(), mlua::Error> = pm.lua.load(r#"zoi.set_data("bad", 0/0)"#).exec();
    assert!(
        result.is_err(),
        "set_data should reject non-finite numbers instead of panicking"
    );
}

#[test]
fn test_plugin_load_all_uses_deterministic_sorted_order() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("Failed to create temp dir");
    let root = tmp.path().to_path_buf();

    ctx.set_env_var("HOME", root.clone());

    let plugin_dir = root.join(".zoi/plugins");
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(
        plugin_dir.join("z-last.lua"),
        r#"
zoi.on_project_install(function()
    zoi.set_data("plugin_order_winner", "z-last")
    return true
end)
"#,
    )
    .unwrap();
    fs::write(
        plugin_dir.join("a-first.lua"),
        r#"
zoi.on_project_install(function()
    zoi.set_data("plugin_order_winner", "a-first")
    return true
end)
"#,
    )
    .unwrap();

    let pm = plugin::PluginManager::new().unwrap();
    pm.load_all().unwrap();

    let handled = pm.trigger_project_install_hook().unwrap();
    assert!(
        handled,
        "the first registered plugin hook should handle the install"
    );

    let winner: Option<String> = pm
        .lua
        .load(r#"return zoi.get_data("plugin_order_winner")"#)
        .eval()
        .unwrap();
    assert_eq!(winner.as_deref(), Some("a-first"));
}

#[test]
fn test_plugin_on_project_install_precedence() {
    let pm = plugin::PluginManager::new().expect("Failed to create PluginManager");

    pm.lua
        .load(
            r#"
        local run_count = 0
        zoi.on_project_install(function()
            run_count = run_count + 1
            return true
        end)
        zoi.on_project_install(function()
            run_count = run_count + 1
            return true
        end)
        
        function get_count() return run_count end
    "#,
        )
        .exec()
        .expect("Failed to load lua");

    let handled = pm.trigger_project_install_hook().expect("Hook failed");
    assert!(handled);

    let count: i32 = pm.lua.load("return get_count()").eval().unwrap();
    assert_eq!(
        count, 1,
        "Only the first hook that returns true should execute"
    );
}
