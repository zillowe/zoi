---
title: Library
description: How to integrate Zoi's package management features into your own Rust applications.
---

Zoi is not just a command-line tool; it's also a Rust library that you can use to programmatically install and manage packages. This allows you to embed Zoi's powerful package management capabilities directly into your own applications, such as custom development tools, game launchers, or server deployment scripts.

## Getting Started

To use Zoi as a library, add it as a dependency in your project

```sh
cargo add zoi-rs
```

Or in your `Cargo.toml` file:

```toml
[dependencies]
zoi-rs = { version = "4.2.0-beta-prod" } # subject to change
```

The main entry point for all installation logic is the `zoi::install` function.

## The `install` Function

This function handles the entire installation process for a given package, including resolving the package source, handling dependencies, and running any necessary installation steps.

### Signature

```rust
pub fn install(
    source: &str,
    mode: InstallMode,
    force: bool,
    reason: InstallReason,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>>
```

### Parameters

| Parameter | Type            | Description                                                                                                                                                   |
| --------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `source`  | `&str`          | The identifier for the package to install. This can be a package name (e.g. `"hello"`), a path to a local `.pkg.yaml` file, or a URL to a package definition. |
| `mode`    | `InstallMode`   | An enum that specifies the preferred installation method.                                                                                                     |
| `force`   | `bool`          | If `true`, Zoi will reinstall the package even if it is already present.                                                                                      |
| `reason`  | `InstallReason` | An enum indicating why the package is being installed. This is typically `Direct` for user-initiated installs.                                                |
| `yes`     | `bool`          | If `true`, Zoi will automatically answer "yes" to any confirmation prompts, making the installation non-interactive.                                          |

### The `InstallMode` Enum

This enum controls which installation method Zoi will attempt to use.

| Variant        | Description                                                                                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PreferBinary` | **(Default)** Zoi will first attempt to download a pre-compiled binary. If one is not available for the target platform, it will fall back to building from source. |
| `ForceSource`  | Zoi will always build the package from source, even if a pre-compiled binary is available.                                                                          |
| `Interactive`  | If multiple installation methods are available, Zoi will prompt the user to choose which one to use.                                                                |

### The `InstallReason` Enum

This enum is used to track why a package was installed, which helps with managing dependencies.

| Variant      | Description                                                                 |
| ------------ | --------------------------------------------------------------------------- |
| `Direct`     | The package was installed directly by a user's request.                     |
| `Dependency` | The package was installed automatically as a dependency of another package. |

For complete, runnable examples, please see the [Library Examples page](/docs/zds/zoi/lib/examples/).
