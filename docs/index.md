---
title: Zoi
description: Universal Package Manager & Environment Setup Tool.
---

This guide will provide you with everything you need to know to get started, from installation to advanced usage.

[Repository](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi)

## Introduction

Zoi is a universal package manager and environment setup tool, designed to simplify package management and environment configuration across multiple operating systems. It's part of the [Zillowe Development Suite (ZDS)](/docs/zds/) and aims to streamline your development workflow by managing tools and project environments with ease.

## Features

- **Cross-Platform:** Works seamlessly on Linux, macOS, and Windows.
- **Universal Package Support:** Install packages from various sources: binaries, compressed archives, build from source, or installer scripts.
- **Extensive Dependency Management:** Integrates with over 30+ package managers (`apt`, `brew`, `cargo`, `npm`, `pip`, `scoop`, etc.) to handle dependencies.
- **Rich Dependencies:** Packages can define runtime and build dependencies with required, optional, and selectable options groups.
- **Project Environments:** Easily define and manage project-specific environments and commands using [`zoi.yaml`](/docs/zds/zoi/project-config/).
- **Repository-Based:** Manage packages from official or community repositories. Easily add your own.
- **Intuitive CLI:** A simple and powerful command-line interface with helpful aliases for a better developer experience.
- **Package Types:** Supports standard packages, meta-packages (collections), background services, configuration packages, extensions, libraries, and app templates.
- **Secure Package Distribution:** Support for checksums and GPG signatures to verify package integrity and authenticity.
- **Tag-based Discovery:** Search by and filter packages using tags for faster discovery.
- **Use as a Library:** Integrate Zoi's package management features directly into your Rust applications. See the [Library documentation](/docs/zds/zoi/lib/) for details.
- **Package Recording & Re-installation:** Automatically keeps a record of all installed packages, which can be used to easily reinstall them on a new machine.

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

#### From Crates.io

You can install `zoi` directly from [crates.io](https://crates.io/crates/zoi-rs) using `cargo`

```sh
cargo install zoi-rs
```

#### From NPM

You can install `@zillowe/zoi` from `npm` also.

<Tabs defaultValue="npm">
  <TabsList>
    <TabsTrigger value="npm">
      <FaNpm />
      npm
    </TabsTrigger>
    <TabsTrigger value="bun">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width={24}
        height={24}
        fill={"currentColor"}
        viewBox="0 0 24 24"
      >
        <path d="m9.34,7.66c-.07.06-.02.18.08.15.85-.33,2.01-1.32,1.52-3.33-.02-.11-.17-.08-.17.02,0,1.21-.51,2.36-1.43,3.16Z"></path>
        <path d="m11.34,4.51c.63,1.02.78,2.27.41,3.41-.03.09.08.16.14.09.55-.71,1.04-2.12-.41-3.64-.07-.07-.19.04-.14.12h0Z"></path>
        <path d="m12.04,4.46c.98.69,1.61,1.78,1.73,2.98,0,.05.03.09.07.09.04,0,.08-.02.09-.07.23-.88.1-2.39-1.82-3.17-.1-.04-.17.1-.08.16h0Z"></path>
        <path d="m7.37,7.26c.91.06,2.38-.37,2.82-2.39.02-.1-.12-.14-.17-.05-.52,1.1-1.48,1.93-2.65,2.28-.09.03-.09.16,0,.16Z"></path>
        <path d="m19.87,8.36s-.08-.09-.13-.13c-.04-.04-.08-.09-.13-.13-.04-.04-.08-.09-.13-.13-.04-.04-.08-.09-.13-.13-.04-.04-.08-.09-.13-.13-.04-.04-.08-.09-.13-.12h0s0,0,0,0c-.02-.02-.04-.04-.06-.06-.62-.65-1.37-1.23-2.25-1.7-.94-.5-1.65-.94-2.28-1.33-.27-.17-.52-.33-.78-.47-.83-.49-1.49-.79-2.25-.79s-1.52.35-2.41.88c-.29.17-.59.36-.91.55-.58.36-1.23.76-1.99,1.17-2.62,1.42-4.18,3.78-4.18,6.31,0,1.86.84,3.56,2.22,4.88l.1.1.76.76c1.74,1.75,4.42,2.88,7.43,2.88,5.24,0,9.49-3.41,9.49-7.59,0-1.79-.77-3.48-2.13-4.81Zm-8.38,10.76c-4.89,0-8.86-3.13-8.86-6.97,0-2.3,1.44-4.45,3.85-5.75.76-.41,1.44-.83,2.03-1.19.31-.19.61-.38.9-.54.82-.48,1.48-.79,2.09-.79s1.17.25,1.94.71c.23.14.48.3.76.47.61.37,1.35.85,2.31,1.36,2.41,1.3,3.85,3.45,3.85,5.75,0,3.85-3.97,6.97-8.86,6.97Z"></path>
        <path d="m13.08,13.65h-3.12c-.11,0-.22.04-.29.13-.06.07-.08.16-.07.25.12.5.4.95.79,1.28.31.29.7.47,1.13.51.42-.04.82-.22,1.12-.51.39-.33.67-.78.79-1.28.02-.09,0-.18-.06-.25-.07-.08-.18-.13-.29-.13Zm-1.56,1.87c-.35-.04-.67-.2-.92-.44-.02-.02-.04-.04-.06-.06.26-.33.65-.52,1.07-.53.38.01.74.16,1.01.43-.05.06-.11.12-.17.17-.25.24-.58.4-.92.44Z"></path>
        <path d="m8.38,10.14c-.77,0-1.4.62-1.4,1.39h0c0,.77.62,1.4,1.39,1.4.77,0,1.4-.62,1.4-1.39,0-.77-.62-1.4-1.39-1.4Zm-.44,1.48c-.29,0-.52-.24-.52-.52h0c0-.29.24-.53.53-.52.29,0,.52.24.52.53,0,.29-.24.52-.53.52Z"></path>
        <path d="m14.67,10.14c-.77,0-1.4.61-1.41,1.38,0,0,0,0,0,.01,0,.77.62,1.39,1.38,1.39.77,0,1.4-.61,1.41-1.38s-.61-1.4-1.38-1.41Zm-.45,1.48h0c-.29,0-.52-.24-.52-.52h0c0-.29.24-.53.53-.52.29,0,.52.24.52.53,0,.29-.24.52-.53.52Z"></path>
      </svg>
      bun
    </TabsTrigger>
    <TabsTrigger value="pnpm">
      <TbBrandPnpm />
      pnpm
    </TabsTrigger>
    <TabsTrigger value="yarn">
      <FaYarn />
      yarn
    </TabsTrigger>
  </TabsList>
  <TabsContent value="npm">

    ```sh
    npx @zillowe/zoi
    ```

  </TabsContent>
  <TabsContent value="bun">

    ```sh
    bunx @zillowe/zoi
    ```

</TabsContent>
  <TabsContent value="pnpm">

    ```sh
    pnpm dlx @zillowe/zoi
    ```

</TabsContent>
  <TabsContent value="yarn">

    ```sh
    yarn dlx @zillowe/zoi
    ```

</TabsContent>
</Tabs>

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
# For Linux/macOS
./build/build-release.sh
# For Windows
./build/build-release.ps1

# Install it locally
./configure
make
sudo make install
# Install CLI completions (bash, zsh, fish, elvish, powershell)
make install-completion
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

Zoi provides a wide range of commands to manage your packages and environment. For a full list of commands and their options, you can always run `zoi --help`.

### General Commands

| Command      | Description                                                                                                                                                                                                                                                                                                                                          |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`    | Displays the version number, build status, branch, and commit hash.                                                                                                                                                                                                                                                                                  |
| `about`      | Displays the full application name, description, author, license, and homepage.                                                                                                                                                                                                                                                                      |
| `info`       | Displays key system details like OS, CPU architecture, and available package managers (requires `zoi sync` to be run first for package manager detection).                                                                                                                                                                                           |
| `check`      | Verifies that all required dependencies (like git) are installed.                                                                                                                                                                                                                                                                                    |
| `setup`      | Configures the shell environment for Zoi to make package binaries available. <br/>`--scope <user\|system>`: Set the scope (default: user).                                                                                                                                                                                                           |
| `sync`       | Clones or updates the package database from the configured registry. <br/>`--verbose`: Show the full git output.                                                                                                                                                                                                                                     |
| `upgrade`    | Downloads the latest release of Zoi and replaces the current executable. <br/>`--full`: Force a full download, skipping the patch-based upgrade. <br/>`--force`: Force the upgrade even if the version is the same. <br/>`--tag <tag>`: Upgrade to a specific git tag. <br/>`--branch <branch>`: Upgrade to the latest release of a specific branch. |
| `clean`      | Clears the cache of downloaded package binaries.                                                                                                                                                                                                                                                                                                     |
| `autoremove` | Removes packages that were installed as dependencies but are no longer needed.                                                                                                                                                                                                                                                                       |
| `telemetry`  | Manages [opt-in telemetry](./telemetry).                                                                                                                                                                                                                                                                                                             |

### Package Management

| Command      | Description                                                                                                                                                                                                                                                                                                           |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list`       | Lists installed or all available packages. <br/>`--all`: List all packages, not just installed. <br/>`--repo <repo>`: Filter by repository. <br/>`--type <type>`: Filter by package type.                                                                                                                             |
| `show`       | Shows detailed information about a package. <br/>`--raw`: Display the raw, unformatted package file.                                                                                                                                                                                                                  |
| `search`     | Searches packages by name, description, and tags. <br/>`--repo <repo>`: Filter by repository. <br/>`--type <type>`: Filter by package type. <br/>`-t, --tag <tag>`: Filter by one or more tags (repeat or comma-separate).                                                                                            |
| `shell`      | Installs completion scripts for a given shell (e.g. `zoi shell bash`).                                                                                                                                                                                                                                                |
| `install`    | Installs one or more packages. Can also install from a `zoi.pkgs.json` file (to restore all packages) or a `<pkg>.manifest.yaml` file (for reproducible dependency choices). <br/>`--force`: Force re-installation if the package already exists. <br/>`--interactive`: Choose the installation method interactively. |
| `build`      | Builds and installs one or more packages from source. <br/>`--force`: Force the package to be rebuilt.                                                                                                                                                                                                                |
| `uninstall`  | Removes one or more packages. Also removes any of its dependencies that are no longer needed. For collections, it removes all of its dependencies.                                                                                                                                                                    |
| `update`     | Updates one or more packages to the latest version.                                                                                                                                                                                                                                                                   |
| `pin`        | Pins a package to a specific version to prevent updates.                                                                                                                                                                                                                                                              |
| `unpin`      | Unpins a package, allowing it to be updated again.                                                                                                                                                                                                                                                                    |
| `rollback`   | Rolls back a package to its previously installed version. See [Package Rollbacks](/docs/zds/zoi/rollbacks/).                                                                                                                                                                                                          |
| `why`        | Explains why a package is installed (e.g. as a dependency or directly).                                                                                                                                                                                                                                               |
| `clone`      | Clones the source code repository of one or more packages. A target directory can only be specified when cloning a single package.                                                                                                                                                                                    |
| `exec`       | Downloads a binary to a temporary cache and runs it without installing it.                                                                                                                                                                                                                                            |
| `create`     | Creates an application from an app template. Usage: `zoi create <source> <appName>`                                                                                                                                                                                                                                   |
| `make`       | Interactively creates a new `pkg.yaml` file.                                                                                                                                                                                                                                                                          |
| `pkg-config` | Provides compiler and linker flags for installed libraries.                                                                                                                                                                                                                                                           |

### Project Environment

| Command | Description                                                                    |
| ------- | ------------------------------------------------------------------------------ |
| `run`   | Executes a command from a local `zoi.yaml` file. Can be run interactively.     |
| `env`   | Sets up project environments from a `zoi.yaml` file. Can be run interactively. |

See the full schema and examples in [Project Configuration (zoi.yaml)](/docs/zds/zoi/project-config/).

### Service Management

| Command | Description                         |
| ------- | ----------------------------------- |
| `start` | Starts a package that is a service. |
| `stop`  | Stops a running service package.    |

### Sync Management (`sync`)

Manages the package database registry URL.

| Subcommand                  | Description                                                                                  |
| --------------------------- | -------------------------------------------------------------------------------------------- |
| `sync set <url or keyword>` | Sets the package database registry URL. Keywords: `default`, `gitlab`, `github`, `codeberg`. |
| `sync show`                 | Displays the current registry URL.                                                           |

### Repository Management (`repo`)

Manages the list of package repositories that Zoi uses.

| Subcommand                | Description                                                                     |
| ------------------------- | ------------------------------------------------------------------------------- |
| `repo add <name-or-url>`  | Add an official repo by name or clone from a git URL (interactive if no args).  |
| `repo rm <name>`          | Remove a repository from the active list.                                       |
| `repo ls`                 | Show active repositories. Use `repo ls all` to show all available repositories. |
| `repo git ls`             | List cloned git repositories under `~/.zoi/pkgs/git`.                           |
| `repo git rm <repo-name>` | Remove a cloned git repository directory (`~/.zoi/pkgs/git/<repo-name>`).       |

**Example:**

```sh
# Add a repository interactively
zoi repo add

# Add a repository by name
zoi repo add community

# Add a repository by git URL (auto-clone)
zoi repo add https://example.com/my-zoi-repo.git

# Remove a repository
zoi repo rm community

# List active repositories
zoi repo ls
```

For an overview of official repositories, mirrors, and repository tiers, see [Repositories](/docs/zds/zoi/repositories/).

Zoi supports different types of packages, defined in the `.pkg.yaml` file.

| Type         | Description                                                                                                                                                                   |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Package`    | A standard software package that can be installed. This is the default type.                                                                                                  |
| `Collection` | A meta-package that groups other packages together as dependencies.                                                                                                           |
| `Service`    | A package that runs as a background service. Can be managed with shell commands or via Docker Compose.                                                                        |
| `Config`     | A package that manages configuration files. It includes commands for installing and uninstalling the configuration.                                                           |
| `App`        | An app template. Not installable; used via `zoi create` to scaffold an application (e.g. frameworks like Rails).                                                              |
| `Extension`  | A package that can modify Zoi's configuration, such as adding new package repositories. It is not installed in the traditional sense but its changes are applied or reverted. |
| `Library`    | A software library with headers and/or binaries (.so, .dll, .a). Can provide pkg-config files.                                                                                |

## Creating Packages (`pkg.yaml`)

Creating a package for Zoi is done by defining a `pkg.yaml` file. This file contains all the metadata and instructions Zoi needs to install your software. For more examples, see the [Package Examples](/docs/zds/zoi/examples/) page.

### `pkg.yaml` Structure

Here is a comprehensive overview of the fields available in a `pkg.yaml` file. For validation and autocompletion in your editor, you can add a `$schema` line pointing to the official JSON Schema.

```yaml
# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json
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
# (Optional) A URL to a README file for the package.
readme: https://my-awesome-app.com/README.md
# (Optional) The source code repository URL.
git: https://github.com/user/my-awesome-app
# Information about the package maintainer.
maintainer:
  name: "Your Name"
  email: "your@email.com"
  # (Optional) URL to the maintainer's public GPG key or the 40-character fingerprint.
  key: "DEADC0DEDEADBEEFDEADC0DEDEADBEEFDEADC0DE"
  # (Optional) Website of the maintainer
  website: "https://maintainer.com"
# (Optional) Information about the original author, if different from the maintainer.
author:
  name: "Author Name"
  email: "author@email.com"
  website: "https://author.com"
  # (Optional) URL to the author's public GPG key or the 40-character fingerprint.
  key: "https://example.com/author.gpg"
# (Optional) The license of the package. Should be a valid SPDX license expression
# (e.g. "MIT", "GPL-3.0-or-later", "MIT OR Apache-2.0").
# Use "Proprietary" for non-SPDX/proprietary licenses.
# Zoi will check for OSI-approved licenses and warn if the check fails.
license: MIT
# (Optional) The installation scope. Can be 'user' (default) or 'system'.
scope: user
# (Optional) The package type. Can be 'package' (default), 'collection', 'service', or 'config'.
type: package
# (Optional) An alternate source to resolve this package from. Can be another
# package name (e.g. 'my-app-git'), a URL to a raw .pkg.yaml file, or a local file path.
alt: my-app-v2
# (Optional) The installation method to use for `zoi update`.
# Can be 'binary', 'com_binary', 'script', or 'source'.
updater: binary
# (Optional) Enable or disable rollbacks for this package. Defaults to true.
# This overrides the global setting in Zoi's config.
rollback: true

# (Optional) A list of important messages to show to the user before installation.
updates:
  - type: change
    message: "This version introduces a new configuration file format."
  - type: vulnerability
    message: "A critical vulnerability (CVE-2025-12345) is present in this version."
  - type: update
    message: "This package will no longer be maintained after 2026."

# (Optional) A list of binaries this package provides. Used for conflict detection.
bins:
  - my-awesome-app
# (Optional) A list of package names that this package conflicts with.
conflicts:
  - my-awesome-app-legacy

# (Optional) A list of tags for discovery and filters.
tags:
  - cli
  - devtools

# (Optional) Defines this package as an extension.
extension:
  type: zoi # The type of extension, currently only 'zoi' is supported.
  changes:
    # A list of changes to apply.
    - type: repo-git # Clones a git repository into the git package source.
      add: https://github.com/user/my-zoi-pkgs.git
    - type: registry-repo # Sets the main package database registry URL.
      add: https://my-mirror.com/Zoi-Pkgs.git
    - type: repo-add # Adds an official repository to the active list.
      add: my-custom-repo

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
    # (Optional) GPG signature verification for the downloaded file.
    # Zoi will use the 'key' from the 'maintainer' or 'author' fields.
    sigs:
      - file: "my-awesome-app-{platform}"
        sig: "https://github.com/user/my-awesome-app/releases/download/v{version}/my-awesome-app-{platform}.sig"
    # (Optional) A list of installer/bundle types like 'dmg', 'msi', or 'appimage'.
    # Zoi will handle these file types with appropriate platform-specific actions.
    binary_types: ["dmg", "msi", "appimage"]

  - type: com_binary
    url: "https://github.com/user/my-awesome-app/releases/download/v{version}/my-awesome-app-v{version}-{platform}.{platformComExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    platformComExt:
      linux: tar.zst
      macos: tar.zst
      windows: zip
    # (Optional) The path or filename of the binary inside the archive.
    # If it ends with .exe, Zoi will name the installed file <package>.exe; otherwise <package>.
    # On Windows targets, if binary_path does not end with .exe, Zoi will also try with .exe appended.
    # Examples:
    # binary_path: "bin/my-awesome-app"
    # binary_path: "my-awesome-app.exe"
    binary_path: "bin/my-awesome-app"

  - type: source
    url: "https://github.com/{git}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # (Optional) Use only one of these. {version} will be expanded.
    # tag: "v{version}"
    # branch: "release/{version}"
    commands:
      - "make build"
      - "mv ./bin/my-awesome-app {store}/my-awesome-app"

# (Optional) Dependencies required by the package.
dependencies:
  # Dependencies are split into 'build' and 'runtime'.
  build:
    required:
      - native:cmake
    optional:
      - native:doxygen:for generating documentation
  runtime:
    required:
      # Simple required dependency
      - zoi:another-zoi-package
      # A group of selectable required dependencies
      options:
        - name: "GUI Toolkit"
          desc: "Choose a GUI provider"
          all: no # User must choose one
          depends:
            - native:qt6:for KDE desktops
            - native:gtk4:for GNOME desktops
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

# (Optional) Post-uninstallation commands to run before the package is removed.
post_uninstall:
  - platforms: ["linux", "macos"]
    commands:
      - "rm -f ~/.local/share/bash-completion/completions/{name}"
      - "rm -f ~/.zsh/completions/_{name}"
      - "rm -f ~/.config/fish/completions/{name}.fish"
```

### Installation Methods

Zoi supports four types of installation methods within the `installation` list:

1.  **`binary`**: Downloads a pre-compiled binary directly from a URL.
2.  **`com_binary`**: Downloads a compressed archive (`.zip`, `.tar.gz`, etc.), extracts it, and finds the binary within. Supports `binary_path` to point to the executable inside the archive. On Windows targets, if `binary_path` lacks `.exe`, Zoi will also try with `.exe` appended.
3.  **`source`**: Clones a git repository and runs a series of build commands you define. Supports optional `tag` or `branch` (use only one). If none is provided, the default branch HEAD is used. `tag` can include placeholders like `v{version}`.
4.  **`script`**: Downloads and executes an installation script (e.g. `install.sh`).

For the list of supported archive formats for `com_binary`, see [Supported Archives for Compressed Binaries](/docs/zds/zoi/archives/).

### Dependencies

For the full list of supported dependency managers, usage semantics, and commands Zoi runs, see [Dependencies & Supported Package Managers](/docs/zds/zoi/dependencies/).

## FAQ

<Accordions type="single">
  <Accordion title="How do I create my own package for Zoi?">
    You can create a `pkg.yaml` file that defines your package. This file
    includes metadata like the package name, version, description, and
    installation instructions. The `Package` struct in [`types.rs`](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/src/pkg/types.rs) shows
    all available fields, and there's a [schema](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/blob/main/app/pkg.schema.json) you can use in your `pkg.yaml` file.
    For more info about creating and publishing a package please visit [this guide](/docs/zds/zoi/creating-packages/).
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do optional dependencies work?">
    You can specify `optional` dependencies in your `pkg.yaml` under the `build`
    or `runtime` sections. When a user installs your package, they will be shown
    the list of optional dependencies and their descriptions, and they can choose which ones to install.
    This is great for plugins or extra features.
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
<br />
<Accordions type="single">
  <Accordion title="How do I install a specific version or channel?">
    You can append a version or channel after an '@' in the source string.
    Examples: `zoi install my-app@1.2.3` or use channels defined in
    `versions` like `my-app@stable`. You can also pin a resolved
    version to prevent updates: `zoi pin my-app VERSION` and later
    `zoi unpin my-app`.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I run a tool without installing it?">
    Use `zoi exec <package>`. Zoi will fetch a temporary binary and run it
    without adding it to your PATH or installed set.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I use a custom git package repository?">
    Add it with `zoi repo add https://github.com/you/your-zoi-repo.git`.
    Install packages from it using `@git/<repo-name>/<pkg-name>`.
    List/remove cloned git repos with `zoi repo git ls` and
    `zoi repo git rm <repo-name>`.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="Where are binaries installed and how do I fix PATH?">
    Binaries are placed in `~/.zoi/pkgs/store/<name>/bin` and linked into
    `~/.zoi/pkgs/bin`. Zoi attempts to add this to your shell PATH on Unix and user PATH on Windows.
    If commands are not found, run `zoi setup` to configure your shell, or add `~/.zoi/pkgs/bin` to your PATH manually.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I accept all prompts (non-interactive)?">
    Pass `--yes` to installation commands to auto-accept confirmations and optional dependency prompts.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I install from a nested repository path?">
    Use the full path in the source, e.g. `@core/linux/amd64/nvidia-driver`.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I update Zoi itself?">
    Run `zoi upgrade`. Zoi attempts a patch (delta) upgrade first and falls back to a full binary download if needed. You can use `zoi upgrade --full` to force a full download, and `zoi upgrade --force` to upgrade even if you are on the latest version.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I rollback a package update?">
    If an update causes issues, you can revert to the previously installed version using `zoi rollback <package-name>`. Zoi automatically creates a backup before upgrading a package. For more details, see the [Package Rollbacks](/docs/zds/zoi/rollbacks/) guide.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How can I back up and restore my installed packages?">
    Zoi automatically keeps a record of all your installed packages in `~/.zoi/pkgs/zoi.pkgs.json`. You can back up this file. To restore all your packages on a new system, simply run `zoi install /path/to/your/zoi.pkgs.json`.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="How do I ensure a reproducible installation?">
    When you install a package with selectable or optional dependencies, Zoi saves your choices to a `<package_name>.manifest.yaml` file in the package's installation directory. You can commit this file to your project and run `zoi install /path/to/your/package.manifest.yaml` to reinstall the package with the exact same dependencies non-interactively.
  </Accordion>
</Accordions>
<br />
<Accordions type="single">
  <Accordion title="Does Zoi collect your data?">
    We believe in privacy by design. Zoi does **not** collect any data by default. You can choose to opt-in to our anonymous telemetry to help us improve the application. To learn more about what we collect and why, please see our [Telemetry & Analytics](/docs/zds/zoi/telemetry/) page.
  </Accordion>
</Accordions>

## Examples

- **Install a package:**

  ```sh
  zoi install <package_name>
  ```

- **Install multiple packages:**

  ```sh
  zoi install <package_1> <package_2>
  ```

- **Update a package:**

  ```sh
  zoi update <package_name>
  ```

- **Update multiple packages:**

  ```sh
  zoi update <package_1> <package_2>
  ```

- **Update all packages:**

  ```sh
  zoi update all
  ```

- **Uninstall a package:**

  ```sh
  zoi uninstall <package_name>
  ```

- **Uninstall multiple packages:**

  ```sh
  zoi uninstall <package_1> <package_2>
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

- **Search by tag directly:**

  ```sh
  # The term matches tags too
  zoi search editor

  # Require specific tag(s)
  zoi search editor -t cli,devtools
  zoi search editor -t cli -t devtools
  ```

- **Check why a package is installed:**

  ```sh
  zoi why <package_name>
  ```

- **Create an app from a template:**
  ```sh
  zoi create <source> <appName>
  # e.g.
  zoi create rails-app MyBlog
  zoi create @community/rails-app MyBlog
  ```
