//! `system.lua` settings applied to a fake root.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use zoi_system::config::{
    FilesystemConfig, KeyboardConfig, ServiceConfig, SystemConfig
};
use zoi_system::settings::{apply_settings, normalize_guest_path};

/// A root with the data the settings check against: one time zone, a
/// glibc locale list, one keymap and one console font, plus an existing
/// `/etc/hosts` and `/etc/locale.gen` with lines of their own.
fn fake_root() -> TempDir {
    let root = TempDir::new().expect("temp root");
    let put = |path: &str, content: &str| {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, content).expect("write");
    };
    put("usr/share/zoneinfo/Europe/Berlin", "TZif");
    put(
        "usr/share/i18n/SUPPORTED",
        "de_DE.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\nen_US ISO-8859-1\n"
    );
    put("usr/share/kbd/keymaps/i386/qwertz/de.map.gz", "");
    put("usr/share/kbd/consolefonts/ter-v16n.psf.gz", "");
    put("etc/hosts", "127.0.0.1 localhost\n");
    put("etc/locale.gen", "# Configuration file for locale-gen\n");
    root
}

/// A config using every setting.
fn full_config() -> SystemConfig {
    let mut config = SystemConfig::default();
    let system = &mut config.system;
    system.hostname = Some("box.example.org".to_string());
    system.timezone = Some("Europe/Berlin".to_string());
    system.locale = Some("en_US.UTF-8".to_string());
    system.locale_settings = Some(BTreeMap::from([(
        "LC_TIME".to_string(),
        "de_DE.UTF-8".to_string()
    )]));
    system.keymap = Some("de".to_string());
    system.console_font = Some("ter-v16n".to_string());
    system.keyboard = Some(KeyboardConfig {
        layout: Some("de".to_string()),
        options: Some("ctrl:nocaps".to_string()),
        ..KeyboardConfig::default()
    });
    system.ntp = Some(true);
    system.network = Some("networkd".to_string());
    config.services.insert(
        "systemd-resolved.service".to_string(),
        ServiceConfig { enable: false }
    );
    config.filesystems.push(FilesystemConfig {
        device: "LABEL=zoios".to_string(),
        mount: "/".to_string(),
        fs_type: "ext4".to_string(),
        options: None
    });
    config
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).expect(path)
}

#[test]
fn every_setting_is_written_under_the_root() {
    let root = fake_root();
    let r = root.path();
    let report = apply_settings(r, &full_config(), false).expect("apply");

    assert_eq!(read(r, "etc/hostname"), "box.example.org\n");
    let hosts = read(r, "etc/hosts");
    assert!(
        hosts.starts_with("127.0.0.1 localhost\n"),
        "keeps own lines"
    );
    assert!(hosts.contains("127.0.1.1        box.example.org box\n"));
    assert_eq!(
        fs::read_link(r.join("etc/localtime")).expect("localtime link"),
        PathBuf::from("../usr/share/zoneinfo/Europe/Berlin")
    );
    let locale_gen = read(r, "etc/locale.gen");
    assert!(locale_gen.starts_with("# Configuration file for locale-gen\n"));
    assert!(locale_gen.contains("\nen_US.UTF-8 UTF-8\nde_DE.UTF-8 UTF-8\n"));
    assert_eq!(
        read(r, "etc/locale.conf"),
        "LANG=en_US.UTF-8\nLC_TIME=de_DE.UTF-8\n"
    );
    assert_eq!(read(r, "etc/vconsole.conf"), "KEYMAP=de\nFONT=ter-v16n\n");
    let keyboard = read(r, "etc/X11/xorg.conf.d/00-keyboard.conf");
    assert!(keyboard.contains("Option \"XkbLayout\" \"de\"\n"));
    assert!(keyboard.contains("Option \"XkbOptions\" \"ctrl:nocaps\"\n"));
    assert!(!keyboard.contains("XkbVariant"));
    assert!(
        read(r, "etc/systemd/network/20-wired.network").contains("DHCP=yes")
    );
    assert_eq!(
        fs::read_link(r.join("etc/resolv.conf")).expect("resolv.conf link"),
        PathBuf::from("/run/systemd/resolve/stub-resolv.conf")
    );
    assert!(read(r, "etc/fstab").contains("LABEL=zoios"));

    // networkd implies networkd + resolved, ntp implies timesyncd, and an
    // explicit services({...}) entry wins.
    assert_eq!(
        report.services,
        BTreeMap::from([
            ("systemd-networkd.service".to_string(), true),
            ("systemd-resolved.service".to_string(), false),
            ("systemd-timesyncd.service".to_string(), true),
        ])
    );
    // The fake root has no locale-gen to compile the locales with.
    assert!(report.warnings.iter().any(|w| w.contains("locale-gen")));
}

#[test]
fn applying_again_changes_nothing() {
    let root = fake_root();
    let config = full_config();
    apply_settings(root.path(), &config, false).expect("first apply");
    let hosts = read(root.path(), "etc/hosts");
    let report = apply_settings(root.path(), &config, false).expect("again");
    assert!(report.changed.is_empty(), "changed: {:?}", report.changed);
    assert_eq!(read(root.path(), "etc/hosts"), hosts, "one managed block");
}

#[test]
fn dry_run_writes_nothing() {
    let root = fake_root();
    let report =
        apply_settings(root.path(), &full_config(), true).expect("dry run");
    assert!(report.changed.contains(&"/etc/hostname".to_string()));
    assert!(!root.path().join("etc/hostname").exists());
    assert!(!root.path().join("etc/localtime").exists());
}

#[test]
fn unknown_or_malformed_values_are_errors() {
    let cases: [(&str, fn(&mut SystemConfig)); 6] = [
        ("time zone", |c| {
            c.system.timezone = Some("Mars/Base".into());
        }),
        ("time zone", |c| {
            c.system.timezone = Some("../etc/passwd".into());
        }),
        ("locale", |c| c.system.locale = Some("xx_XX.UTF-8".into())),
        ("hostname", |c| c.system.hostname = Some("bad_host".into())),
        ("keymap", |c| c.system.keymap = Some("nope".into())),
        ("network", |c| c.system.network = Some("wifi".into()))
    ];
    for (what, set) in cases {
        let root = fake_root();
        let mut config = SystemConfig::default();
        set(&mut config);
        let err = apply_settings(root.path(), &config, false)
            .expect_err(what)
            .to_string();
        assert!(err.to_lowercase().contains(what), "{what}: {err}");
    }
}

#[test]
fn missing_data_packages_are_warnings() {
    let root = TempDir::new().expect("empty root");
    let mut config = SystemConfig::default();
    config.system.timezone = Some("UTC".to_string());
    config.system.keymap = Some("us".to_string());
    let report = apply_settings(root.path(), &config, false).expect("apply");
    assert!(report.warnings.iter().any(|w| w.contains("@core/tzdata")));
    assert!(report.warnings.iter().any(|w| w.contains("@core/kbd")));
    assert!(!root.path().join("etc/localtime").exists());
    // The keymap is still written; only its existence went unchecked.
    assert_eq!(read(root.path(), "etc/vconsole.conf"), "KEYMAP=us\n");
}

#[test]
fn fstab_is_left_alone_without_filesystems() {
    let root = fake_root();
    fs::write(root.path().join("etc/fstab"), "# mine\n").expect("fstab");
    apply_settings(root.path(), &SystemConfig::default(), false)
        .expect("apply");
    assert_eq!(read(root.path(), "etc/fstab"), "# mine\n");
}

#[test]
fn guest_paths_are_normalized_lexically() {
    assert_eq!(
        normalize_guest_path(Path::new(
            "/etc/../run/systemd/resolve/stub-resolv.conf"
        )),
        PathBuf::from("/run/systemd/resolve/stub-resolv.conf")
    );
    assert_eq!(
        normalize_guest_path(Path::new("/../..")),
        PathBuf::from("/")
    );
}
