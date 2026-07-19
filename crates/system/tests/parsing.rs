use std::io::Write;
use tempfile::NamedTempFile;
use zoi_system::config::load_system_lua;

#[test]
fn test_system_lua_parsing() {
    let mut file = NamedTempFile::new().unwrap();
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
    file.write_all(content.as_bytes()).unwrap();

    let config = load_system_lua(file.path()).unwrap();

    assert_eq!(config.system.hostname, Some("test-host".to_string()));
    assert_eq!(config.system.timezone, Some("UTC".to_string()));
    assert_eq!(config.packages.len(), 2);
    assert_eq!(config.packages[0], "@core/bash");
    assert!(config.services.get("sshd").unwrap().enable);
    assert!(!config.services.get("nginx").unwrap().enable);
    assert_eq!(config.filesystems.len(), 1);
    assert_eq!(config.filesystems[0].device, "/dev/sda1");
}
