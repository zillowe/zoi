---
title: Zoi
description: Universal Package Manager & Environment Setup Tool.
---

This guide will provide you with everything you need to know to get started, from installation to advanced usage.

[Repository](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi)

## Introduction

Zoi is a universal package manager and environment setup tool, designed to simplify package management and environment configuration across multiple operating systems. It's part of the Zillowe Development Suite (ZDS) and aims to streamline your development workflow by managing tools and project environments with ease.

## Features

- **Cross-Platform:** Works seamlessly on Linux, macOS, and Windows.
- **Universal Package Support:** Install packages from various sources: binaries, compressed archives, build from source, or installer scripts.
- **Extensive Dependency Management:** Integrates with over 30+ package managers (`apt`, `brew`, `cargo`, `npm`, `pip`, `scoop`, etc.) to handle dependencies.
- **Optional Dependencies:** Packages can define optional dependencies for extra features, which users can select during installation.
- **Project Environments:** Easily define and manage project-specific environments and commands using `zoi.yaml`.
- **Repository-Based:** Manage packages from official or community repositories. Easily add your own.
- **Intuitive CLI:** A simple and powerful command-line interface with helpful aliases for a better developer experience.
- **Package Types:** Supports standard packages, meta-packages (collections), background services, and configuration file management.

## Getting Started

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

## Installation

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

### From Crates.io

You can also install `zoi-cli` directly from [crates.io](https://crates.io/crates/zoi) using `cargo`:

```sh
cargo install zoi-cli
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

## Platforms

What platforms we currently support.

| OS      | Arch  | Zoi Binary | Packages Support |
| ------- | ----- | ---------- | ---------------- |
| Linux   | amd64 | ✔️         | ✔️               |
| Linux   | arm64 | ✔️         | ✔️               |
| macOS   | amd64 | ✔️         | ✔️               |
| macOS   | arm64 | ✔️         | ✔️               |
| Windows | amd64 | ✔️         | ✔️               |
| Windows | arm64 | ❌         | ✔️               |
| FreeBSD | amd64 | ❌         | ✔️               |
| FreeBSD | arm64 | ❌         | ✔️               |
| OpenBSD | amd64 | ❌         | ✔️               |
| OpenBSD | arm64 | ❌         | ✔️               |

We're planning to add support for more platforms.

## Usage & Commands

Zoi provides a wide range of commands to manage your packages and environment.

### General Commands

| Command      | Description                                                                                        |
| ------------ | -------------------------------------------------------------------------------------------------- |
| `version`    | Displays the version number, build status, and branch.                                             |
| `about`      | Displays the full application name, description, author, license, and homepage.                    |
| `info`       | Detects and displays key system details like OS, CPU architecture, and available package managers. |
| `check`      | Verifies that all required dependencies (like git) are installed.                                  |
| `sync`       | Clones or updates the package database from the remote repository.                                 |
| `upgrade`    | Downloads the latest release of Zoi and replaces the current executable.                           |
| `clean`      | Clears the cache of downloaded package binaries.                                                   |
| `autoremove` | Removes packages that were installed as dependencies but are no longer needed.                     |

### Package Management

| Command     | Description                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------- |
| `list`      | Lists installed or all available packages. Use `--all` to see all packages.                 |
| `show`      | Shows detailed information about a package.                                                 |
| `search`    | Searches for a case-insensitive term in the name and description of all available packages. |
| `install`   | Installs a package from a name, local file, or URL.                                         |
| `build`     | Builds and installs a package from a source.                                                |
| `uninstall` | Removes a package's files from the Zoi store.                                               |
| `update`    | Updates a package to the latest version.                                                    |
| `pin`       | Pins a package to a specific version to prevent updates.                                    |
| `unpin`     | Unpins a package, allowing it to be updated again.                                          |
| `why`       | Explains why a package is installed (e.g. as a dependency or directly).                     |
| `clone`     | Clones the source code repository of a package.                                             |
| `exec`      | Downloads a binary to a temporary cache and runs it without installing it.                  |

### Project Environment

| Command | Description                                                            |
| ------- | ---------------------------------------------------------------------- |
| `run`   | Executes a command from a local `zoi.yaml` file.                       |
| `env`   | Manages and sets up project environments from a local `zoi.yaml` file. |

### Repository Management (`repo`)

Manages the list of package repositories that Zoi uses.

| Subcommand    | Description                                                                                                       |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| `repo add`    | Adds a new repository from the available sources or clones a repository from a git URL. Can be run interactively. |
| `repo remove` | Deletes a repository from the active list.                                                                        |
| `repo list`   | Shows all currently active repositories. Use `list --all` to see all available repositories and their status.     |

**Example:**

```sh
# Add a repository interactively
zoi repo add

# Add a repository by name
zoi repo add community

# Add a repository by git URL (auto-clone)
zoi repo add https://example.com/my-zoi-repo.git

# Remove a repository
zoi repo remove community

# List active repositories
zoi repo list
```

Zoi supports different types of packages, defined in the `.pkg.yaml` file.

| Type         | Description                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------- |
| `Package`    | A standard software package that can be installed. This is the default type.                                        |
| `Collection` | A meta-package that groups other packages together as dependencies.                                                 |
| `Service`    | A package that runs as a background service. It includes commands for starting and stopping the service.            |
| `Config`     | A package that manages configuration files. It includes commands for installing and uninstalling the configuration. |

## Creating Packages (`pkg.yaml`)

Creating a package for Zoi is done by defining a `pkg.yaml` file. This file contains all the metadata and instructions Zoi needs to install your software. For more examples, see the [Package Examples](./examples) page.

### `pkg.yaml` Structure

Here is a comprehensive overview of the fields available in a `pkg.yaml` file.

```yaml
# The name of the package. This is required and should be unique.
name: my-awesome-app
# The repository where the package is located (e.g. 'core', 'community', 'core/linux/amd64').
repo: community
# The version of the package. Can be a static version number or a URL to a file containing the version.
version: 1.0.0
# (Optional) A map of version channels to version numbers or URLs.
# Zoi will use the 'stable' channel by default if it exists.
versions:
  stable: 1.0.0
  latest: 1.1.0-beta
# A brief description of the package.
description: My awesome application does awesome things.
# (Optional) The official website for the package.
website: https://my-awesome-app.com
# (Optional) The source code repository URL.
git: https://github.com/user/my-awesome-app
# Information about the package maintainer.
maintainer:
  name: "Your Name"
  email: "your@email.com"
# (Optional) The license of the package.
license: MIT
# (Optional) The installation scope. Can be 'user' (default) or 'system'.
scope: user
# (Optional) The package type. Can be 'package' (default), 'collection', 'service', or 'config'.
type: package

# A list of methods to install the package. Zoi will try them in order.
installation:
  - type: binary
    url: "https://github.com/user/my-awesome-app/releases/download/v{version}/my-awesome-app-{platform}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # (Optional) Checksum verification for the downloaded file.
    checksums:
      # Option A: simple URL to a checksums file (defaults to sha512)
      url: "https://github.com/user/my-awesome-app/releases/download/v{version}/checksums.txt"
      # Option B: explicit list with type (supports sha512 or sha256)
      # type: sha256
      # list:
      #   - file: "my-awesome-app-zip"
      #     checksum: "<hex-digest-or-url>"

# (Optional) Dependencies required by the package.
dependencies:
  # Dependencies are split into 'build' and 'runtime'.
  # Each can have 'required' and 'optional' dependencies.
  build:
    required:
      - native:cmake
    optional:
      - native:doxygen:for generating documentation
  runtime:
    required:
      - zoi:another-zoi-package
    optional:
      - zoi:awesome-plugin:to enable the awesome feature

# (Optional) Post-installation commands to run after a successful installation.
post_install:
  - platforms: ["linux", "macos"]
    commands:
      - "{name} generate-completions bash > ~/.local/share/bash-completion/completions/{name} || true"
      - "{name} generate-completions zsh > ~/.zsh/completions/_{name} || true"
      - "{name} generate-completions fish > ~/.config/fish/completions/{name}.fish || true"
  - platforms: ["windows-amd64"]
    commands:
      - "powershell -NoProfile -Command \"$p=\"$env:USERPROFILE\\Documents\\PowerShell\"; if(!(Test-Path $p)){New-Item -ItemType Directory -Path $p|Out-Null}; {name}.exe generate-completions powershell >> \"$p\\Microsoft.PowerShell_profile.ps1\"\""
```

### Installation Methods

Zoi supports four types of installation methods within the `installation` list:

1.  **`binary`**: Downloads a pre-compiled binary directly from a URL.
2.  **`com_binary`**: Downloads a compressed archive (`.zip`, `.tar.gz`, etc.), extracts it, and finds the binary within.
3.  **`source`**: Clones a git repository and runs a series of build commands you define.
4.  **`script`**: Downloads and executes an installation script (e.g. `install.sh`).

### Supported Dependencies

Zoi can manage dependencies from a wide variety of external package managers. You can specify them in the `build` or `runtime` sections of the `dependencies` map.

The format for a dependency is `manager:package-name`. For optional dependencies, you can add a description like so: `manager:package-name:description`.

| Manager          | Ecosystem / OS                  | Example                               |
| ---------------- | ------------------------------- | ------------------------------------- |
| `zoi`            | Zoi                             | `zoi:my-other-package`                |
| `native`         | System's native package manager | `native:openssl`                      |
| `apt`, `apt-get` | Debian, Ubuntu, etc.            | `apt:libssl-dev`                      |
| `pacman`         | Arch Linux                      | `pacman:base-devel`                   |
| `yay`, `paru`    | Arch Linux (AUR)                | `yay:google-chrome`                   |
| `aur`            | Arch Linux (AUR)                | `aur:visual-studio-code-bin`          |
| `dnf`, `yum`     | Fedora, CentOS, RHEL            | `dnf:openssl-devel`                   |
| `zypper`         | openSUSE                        | `zypper:libopenssl-devel`             |
| `apk`            | Alpine Linux                    | `apk:git`                             |
| `portage`        | Gentoo                          | `portage:dev-libs/openssl`            |
| `brew`           | macOS (Homebrew)                | `brew:node`                           |
| `macports`       | macOS (MacPorts)                | `macports:git`                        |
| `scoop`          | Windows                         | `scoop:git`                           |
| `choco`          | Windows (Chocolatey)            | `choco:git`                           |
| `winget`         | Windows                         | `winget:Git.Git`                      |
| `snap`           | Linux (Snapcraft)               | `snap:node`                           |
| `flatpak`        | Linux (Flathub)                 | `flatpak:org.gimp.GIMP`               |
| `pkg`            | FreeBSD                         | `pkg:git`                             |
| `pkg_add`        | OpenBSD                         | `pkg_add:git`                         |
| `cargo`          | Rust                            | `cargo:ripgrep`                       |
| `cargo-binstall` | Rust (pre-compiled binaries)    | `cargo-binstall:ripgrep`              |
| `go`             | Go                              | `go:golang.org/x/tools/cmd/goimports` |
| `npm`            | Node.js                         | `npm:typescript`                      |
| `yarn`           | Node.js                         | `yarn:react`                          |
| `pnpm`           | Node.js                         | `pnpm:vite`                           |
| `bun`            | Bun                             | `bun:elysia`                          |
| `jsr`            | JavaScript Registry             | `jsr:@std/http`                       |
| `pip`            | Python                          | `pip:requests`                        |
| `pipx`           | Python                          | `pipx:black`                          |
| `conda`          | Conda                           | `conda:numpy`                         |
| `gem`            | Ruby                            | `gem:rails`                           |
| `composer`       | PHP                             | `composer:laravel/installer`          |
| `dotnet`         | .NET                            | `dotnet:csharp-ls`                    |
| `nix`            | NixOS / Nix                     | `nix:nixpkgs.hello`                   |

## FAQ

<Accordions type="single">
  <Accordion title="How do I create my own package for Zoi?">
    You can create a `.pkg.yaml` file that defines your package. This file
    includes metadata like the package name, version, description, and
    installation instructions. The `Package` struct in `src/pkg/types.rs` shows
    all available fields.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do optional dependencies work?">
    You can specify `optional` dependencies in your `pkg.yaml` under the `build`
    or `runtime` sections. When a user installs your package, they will be shown
    the list of optional dependencies and their descriptions, and they can

    choose which ones to install. This is great for plugins or extra features.

  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I add a new repository?">
    You can add a new repository using the `zoi repo add` command. You can run
    it without arguments for an interactive prompt, or provide the name of the
    repository to add it directly.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="What platforms does Zoi support?">
    Zoi is designed to be cross-platform. For a detailed list of supported
    operating systems and architectures, please refer to the
    "[Platforms](#platforms)" section.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="Can I install packages from other package managers?">
    Yes, Zoi supports installing dependencies from a wide range of other package
    managers like `brew`, `winget`, `scoop`, `npm`, `cargo`, `pip`, and many
    more. These are defined in the `dependencies` section of a package's
    `.pkg.yaml` file.
  </Accordion>
</Accordions>

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
  # Install from a top-level repository
  zoi install @community/htop

  # Install from a nested repository
  zoi install @core/linux/amd64/nvidia-driver
  ```

- **List all available packages from active repos:**

  ```sh
  zoi list --all
  ```

- **Search for a package:**

  ```sh
  zoi search <term>
  ```

- **Check why a package is installed:**
  ```sh
  zoi why <package_name>
  ```
