---
title: Package Rollbacks
description: How to roll back a package to a previous version using Zoi.
---

Zoi includes a rollback feature that acts as a safety net, allowing you to quickly revert a package to its previously installed version. This is useful if an update introduces a bug or an unwanted change.

## How It Works

When you install or update a package that is already present on your system, Zoi automatically creates a backup of the existing version before proceeding with the new installation. This backup includes the package's binaries and its manifest file.

The `zoi rollback` command then uses this backup to restore the package to its prior state.

**Key Points:**

- A backup is only created when overwriting an existing installation.
- Only the single most recent version is kept as a backup. Each new update overwrites the previous backup.
- This feature is enabled by default.

## Usage

To roll back a package, simply use the `rollback` command with the package name:

```sh
# Roll back the 'my-cli' package
zoi rollback my-cli
```

Zoi will:

1. Confirm that you want to proceed.
2. Uninstall the current version of the package.
3. Restore the backed-up version.
4. Re-create the necessary symlinks to make the command available in your shell.

## Configuration

You can control the rollback behavior both globally and on a per-package basis.

### Global Configuration

To disable rollbacks for all packages, you can edit your Zoi configuration file located at `~/.zoi/pkgs/config.yaml` and add the following line:

```yaml
# ~/.zoi/pkgs/config.yaml
rollback_enabled: false
```

If the file or key doesn't exist, you can add it.

### Per-Package Configuration

A package author can disable rollbacks for their specific package by adding the `rollback` field to the `pkg.yaml` file and setting it to `false`.

```yaml
# my-package.pkg.yaml
name: my-package
version: 1.2.0
# ... other fields
rollback: false # Disables rollback backups for this package
```

This setting will override the global configuration.

## Limitations

The rollback feature is designed for quick recovery, not for managing multiple versions of a package.

- It only stores **one** previous version.
- To lock a package to a specific version and prevent it from being updated, use the [`zoi pin`](/docs/zds/zoi/) command instead.
