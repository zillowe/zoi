---
title: Zoi Documentation
---

# Zoi Documentation

Welcome to the official documentation for Zoi, the Universal Package Manager & Environment Setup Tool. This guide will provide you with everything you need to know to get started, from installation to advanced usage.

## Introduction

Zoi is a universal package manager and environment setup tool, designed to simplify package management and environment configuration across multiple operating systems. It's part of the Zillowe Development Suite (ZDS) and aims to streamline your development workflow by managing tools and project environments with ease.

## ✨ Features

- **Universal:** Works on Linux, macOS, and Windows.
- **Repository-based:** Manage packages from different sources.
- **Environment Setup:** Configure project environments with ease.
- **Extensible:** Add your own repositories and packages.
- **Simple CLI:** An intuitive and easy-to-use command-line interface.

## 🚀 Getting Started

Getting started with Zoi is simple. Just follow these three steps:

1.  **Install Zoi:**
    Choose one of the [installation methods](#installation) below.

2.  **Sync Repositories:**
    Before you can install packages, you need to sync the package repositories.

    ```sh
    zoi sync
    ```

3.  **Install a Package:**
    Now you can install any package you want. For example, to install `hello`:

    ```sh
    zoi install hello
    ```

## 📦 Installation

You can install Zoi using a package manager, an installer script, or by building it from source.

### Package Managers

#### Arch Linux (AUR)

Install `zoi-bin` (Pre-compiled binary) or `zoi` (built from source) from the AUR:

```sh
yay -S zoi-bin
```

#### macOS (Homebrew)

```sh
brew install Zillowe/tap/zoi
```

#### Windows (Scoop)

```powershell
scoop bucket add zillowe https://github.com/Zillowe/scoop.git
scoop install zoi
```

### Scripts

**Linux / macOS:**

```sh
curl -fsSL https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.sh | bash
```

**Windows:**

```powershell
powershell -c "irm gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/install.ps1|iex"
```

### Build from Source

You'll need [Rust](https://www.rust-lang.org) installed.

```sh
# Build the release binary
./build/build-release.sh # For Linux/macOS
./build/build-release.ps1 # For Windows

# Install it locally
./configure
make
sudo make install
```

## 💡 Usage & Commands

Zoi provides a wide range of commands to manage your packages and environment.

### General Commands

| Command      | Description                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `version`    | Displays the version number, build status, and branch.                                                                                   |
| `about`      | Displays the full application name, description, author, license, and homepage.                                                          |
| `info`       | Detects and displays key system details like OS, CPU architecture, and available package managers.                                         |
| `check`      | Verifies that all required dependencies (like git) are installed.                                                                        |
| `sync`       | Clones or updates the package database from the remote repository.                                                                       |
| `upgrade`    | Downloads the latest release of Zoi and replaces the current executable.                                                                 |
| `clean`      | Clears the cache of downloaded package binaries.                                                                                         |
| `autoremove` | Removes packages that were installed as dependencies but are no longer needed.                                                           |

### Package Management

| Command       | Description                                                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `list`        | Lists installed or all available packages. Use `--all` to see all packages.                                                              |
| `show`        | Shows detailed information about a package.                                                                                              |
| `search`      | Searches for a case-insensitive term in the name and description of all available packages.                                              |
| `install`     | Installs a package from a name, local file, or URL.                                                                                      |
| `build`       | Builds and installs a package from a source.                                                                                             |
| `uninstall`   | Removes a package's files from the Zoi store.                                                                                            |
| `update`      | Updates a package to the latest version.                                                                                                 |
| `pin`         | Pins a package to a specific version to prevent updates.                                                                                 |
| `unpin`       | Unpins a package, allowing it to be updated again.                                                                                       |
| `clone`       | Clones the source code repository of a package.                                                                                          |
| `exec`        | Downloads a binary to a temporary cache and runs it without installing it.                                                               |

### Project Environment

| Command | Description                                                                                                                              |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `run`   | Executes a command from a local `zoi.yaml` file.                                                                                         |
| `env`   | Manages and sets up project environments from a local `zoi.yaml` file.                                                                   |

### Repository Management (`repo`)

Manages the list of package repositories that Zoi uses.

| Subcommand    | Description                                                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `repo add`    | Adds a new repository from the available sources. Can be run interactively.                                                              |
| `repo remove` | Deletes a repository from the active list.                                                                                               |
| `repo list`   | Shows all currently active repositories. Use `list all` to see all available repositories and their status.                                |

**Example:**

```sh
# Add a repository interactively
zoi repo add

# Add a repository by name
zoi repo add community

# Remove a repository
zoi repo remove community

# List active repositories
zoi repo list
```

## Package Types

Zoi supports different types of packages, defined in the `.pkg.yaml` file.

| Type         | Description                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `Package`    | A standard software package that can be installed. This is the default type.                                                             |
| `Collection` | A meta-package that groups other packages together as dependencies.                                                                      |
| `Service`    | A package that runs as a background service. It includes commands for starting and stopping the service.                                   |
| `Config`     | A package that manages configuration files. It includes commands for installing and uninstalling the configuration.                        |

## FAQ

**Q: How do I create my own package for Zoi?**

A: You can create a `.pkg.yaml` file that defines your package. This file includes metadata like the package name, version, description, and installation instructions. The `Package` struct in `src/pkg/types.rs` shows all available fields.

**Q: How do I add a new repository?**

A: You can add a new repository using the `zoi repo add` command. You can run it without arguments for an interactive prompt, or provide the name of the repository to add it directly.

**Q: What platforms does Zoi support?**

A: Zoi is designed to be cross-platform. For a detailed list of supported operating systems and architectures, please refer to the "Platforms" section in the [README](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/README.md#%EF%B8%8F-platforms).

**Q: Can I install packages from other package managers?**

A: Yes, Zoi supports installing dependencies from a wide range of other package managers like `brew`, `winget`, `scoop`, `npm`, `cargo`, `pip`, and many more. These are defined in the `dependencies` section of a package's `.pkg.yaml` file.

## Examples

- **Install a package:**
  ```sh
  zoi install <package_name>
  ```

- **Uninstall a package:**
  ```sh
  zoi uninstall <package_name>
  ```

- **Install from a specific repository:**
  ```sh
  zoi install @<repo_name>/<package_name>
  ```

- **List all available packages from active repos:**
  ```sh
  zoi list --all
  ```

- **Search for a package:**
  ```sh
  zoi search <term>
  ```