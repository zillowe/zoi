//! Integration coverage for `system.lua` parsing.
use std::io::Write;

use tempfile::NamedTempFile;
use zoi_system::config::load_system_lua;

#[test]
fn test_system_lua_parsing() {
    let mut file = NamedTempFile::new().expect("temp file");
    let content = r#"
system({
    hostname = "test-host",
    timezone = "UTC",
    locale = "en_US.UTF-8",
})

packages({
    "@core/bash",
    "@main/vim",
})

services({
    sshd = { enable = true },
    nginx = { enable = false },
})

filesystems({
    {
        device = "/dev/sda1",
        mount = "/",
        type = "ext4",
        options = "noatime",
    },
})
"#;
    file.write_all(content.as_bytes()).expect("write");

    let config = load_system_lua(file.path()).expect("parse");

    assert_eq!(config.system.hostname, Some("test-host".to_string()));
    assert_eq!(config.system.timezone, Some("UTC".to_string()));
    assert_eq!(config.packages.len(), 2);
    assert_eq!(config.packages.first().expect("one package"), "@core/bash");
    assert!(config.services.get("sshd").expect("sshd entry").enable);
    assert!(!config.services.get("nginx").expect("nginx entry").enable);
    assert_eq!(config.filesystems.len(), 1);
    let fs = config.filesystems.first().expect("one filesystem");
    assert_eq!(fs.device, "/dev/sda1");
}
