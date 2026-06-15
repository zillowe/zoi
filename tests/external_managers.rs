use zoi::pkg::dependencies::parse_dependency_string;
use zoi::pkg::pm;

#[test]
fn test_external_manager_command_construction() {
    let dep = parse_dependency_string("apt:curl@7.68.0").expect("should parse");
    assert_eq!(dep.manager, "apt");
    assert_eq!(dep.package, "curl");
    assert_eq!(dep.version_str, Some("7.68.0".to_string()));

    let pm_commands = pm::MANAGERS
        .get(dep.manager)
        .expect("apt should be configured");

    let package_with_version = format!("{}={}", dep.package, dep.version_str.as_ref().unwrap());

    let install_cmd = pm_commands
        .install
        .replace("{package}", dep.package)
        .replace("{package_with_version}", &package_with_version);

    assert_eq!(install_cmd, "apt-get install -y curl=7.68.0");

    let uninstall_cmd = pm_commands.uninstall.replace("{package}", dep.package);

    assert_eq!(uninstall_cmd, "apt-get remove -y curl");
}

#[test]
fn test_external_manager_without_version() {
    let dep = parse_dependency_string("brew:node").expect("should parse");
    assert_eq!(dep.manager, "brew");
    assert_eq!(dep.package, "node");
    assert_eq!(dep.version_str, None);

    let pm_commands = pm::MANAGERS
        .get(dep.manager)
        .expect("brew should be configured");

    let install_cmd = pm_commands
        .install
        .replace("{package}", dep.package)
        .replace("{package_with_version}", dep.package);

    assert_eq!(install_cmd, "brew install node");
}
