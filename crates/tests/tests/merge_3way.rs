use std::fs;
use std::process::Command;
use tempfile::tempdir;
use zoi::Scope;

mod common;

#[test]
fn test_config_3way_merge_integration() {
    let mut ctx = common::TestContextGuard::acquire();
    let tmp = tempdir().expect("failed to create temp dir");
    let root = tmp.path().to_path_buf();

    // 1. Set up a local Zoi registry
    let reg_dir = root.join("my-registry");
    fs::create_dir_all(&reg_dir).unwrap();

    // Initialize registry metadata
    zoi::pkg::registry::init(&reg_dir).unwrap();

    let pkg_name = "test-merge";
    let version_v1 = "1.0.0";
    let version_v2 = "1.1.0";
    let config_rel = "etc/config.txt";

    // Add package v1
    let pkg_repo_dir = reg_dir.join("main").join(pkg_name);
    fs::create_dir_all(&pkg_repo_dir).unwrap();

    let base_config_content = "setting_a = 10\nsetting_b = 20\n";
    fs::write(pkg_repo_dir.join("config.txt"), base_config_content).unwrap();

    let lua_v1 = format!(
        r#"
metadata({{
    name = "{}",
    repo = "main",
    version = "{}",
    description = "test merge",
    maintainer = {{ name = "test", email = "test@example.com" }},
    types = {{ "source" }},
    backup = {{ "{}" }}
}})

function package()
    zcp("${{pkgluadir}}/config.txt", "${{pkgstore}}/{}")
end
"#,
        pkg_name, version_v1, config_rel, config_rel
    );
    fs::write(pkg_repo_dir.join(format!("{}.pkg.lua", pkg_name)), lua_v1).unwrap();

    // Generate metadata and commit to make it a valid git repo
    zoi::pkg::registry::generate_metadata(&reg_dir).unwrap();

    let run_git = |args: &[&str], dir: &std::path::Path| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success());
    };

    run_git(&["init"], &reg_dir);
    run_git(&["config", "user.email", "test@example.com"], &reg_dir);
    run_git(&["config", "user.name", "test"], &reg_dir);
    run_git(&["config", "commit.gpgsign", "false"], &reg_dir);
    run_git(&["config", "init.defaultBranch", "main"], &reg_dir);
    run_git(&["add", "."], &reg_dir);
    run_git(&["commit", "-m", "v1"], &reg_dir);

    // 2. Set up Sysroot and isolated Config
    let sysroot = root.join("sysroot");
    fs::create_dir_all(&sysroot).unwrap();
    ctx.set_sysroot(sysroot.clone());

    let system_config_dir = sysroot.join("etc").join("zoi");
    fs::create_dir_all(&system_config_dir).unwrap();
    fs::write(
        system_config_dir.join("config.yaml"),
        format!(
            "default_registry:
  handle: my-registry
  url: {}
  authorities: []
repos: [main]
added_registries: []
policy:
  default_registry_unoverridable: true
  added_registries_unoverridable: true
",
            reg_dir.to_string_lossy()
        ),
    )
    .unwrap();

    ctx.set_env_var("HOME", root.join("home"));
    ctx.set_current_dir(&root);

    // 3. Sync and Install v1
    zoi::cmd::sync::run(false, false, false, false).unwrap();

    zoi::install_sources(
        &[pkg_name.to_string()],
        &zoi::SourceInstallOptions {
            scope_override: Some(Scope::User),
            yes: true,
            ..Default::default()
        },
    )
    .expect("v1 install failed");

    // Verify .zoiorig exists
    let store_dir = zoi_resolver::local::get_package_version_dir(
        Scope::User,
        "my-registry",
        "main",
        pkg_name,
        version_v1,
    )
    .unwrap();
    let config_path_v1 = store_dir.join(config_rel);
    let orig_path_v1 = config_path_v1.with_extension("txt.zoiorig");
    assert!(
        orig_path_v1.exists(),
        ".zoiorig should be created on install"
    );

    // 4. User modifies config
    let user_modified_content = "setting_a = 99\nsetting_b = 20\n";
    fs::write(&config_path_v1, user_modified_content).unwrap();

    // 5. Update registry to v2
    let upstream_v2_content = "setting_a = 10\nsetting_b = 20\nsetting_c = 30\n";
    fs::write(pkg_repo_dir.join("config.txt"), upstream_v2_content).unwrap();

    let lua_v2 = format!(
        r#"
metadata({{
    name = "{}",
    repo = "main",
    version = "{}",
    description = "test merge",
    maintainer = {{ name = "test", email = "test@example.com" }},
    types = {{ "source" }},
    backup = {{ "{}" }}
}})

function package()
    zcp("${{pkgluadir}}/config.txt", "${{pkgstore}}/{}")
end
"#,
        pkg_name, version_v2, config_rel, config_rel
    );
    fs::write(pkg_repo_dir.join(format!("{}.pkg.lua", pkg_name)), lua_v2).unwrap();

    zoi::pkg::registry::generate_metadata(&reg_dir).unwrap();
    run_git(&["add", "."], &reg_dir);
    run_git(&["commit", "-m", "v2"], &reg_dir);

    // 6. Sync and Update
    zoi::cmd::sync::run(false, false, false, false).unwrap();
    zoi::update_packages(true, &[], true).expect("v2 upgrade failed");

    // 7. Verify result
    let store_dir_v2 = zoi_resolver::local::get_package_version_dir(
        Scope::User,
        "my-registry",
        "main",
        pkg_name,
        version_v2,
    )
    .unwrap();
    let config_path_v2 = store_dir_v2.join(config_rel);

    let actual_content = fs::read_to_string(&config_path_v2).unwrap();
    let expected_content = "setting_a = 99\nsetting_b = 20\nsetting_c = 30\n";
    assert_eq!(
        actual_content, expected_content,
        "3-way merge failed to combine changes correctly"
    );
}
