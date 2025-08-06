---
title: Zoi
description: Universal Package Manager & Environment Setup Tool.
---

This guide will provide you with everything you need to know to get started, from installation to advanced usage.

[Repository](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi)

## Introduction

Zoi is a universal package manager and environment setup tool, designed to simplify package management and environment configuration across multiple operating systems. It's part of the [Zillowe Development Suite (ZDS)](/docs/zds) and aims to streamline your development workflow by managing tools and project environments with ease.

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

#### From Crates.io

You can install `zoi-cli` directly from [crates.io](https://crates.io/crates/zoi-cli) using `cargo`

```sh
cargo install zoi-cli
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
# Install CLI completions (bash, zsh, fist, elvish, powershell)
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

| Command      | Description                                                                                                                                                |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`    | Displays the version number, build status, branch, and commit hash.                                                                                        |
| `about`      | Displays the full application name, description, author, license, and homepage.                                                                            |
| `info`       | Displays key system details like OS, CPU architecture, and available package managers (requires 'zoi sync' to be run first for package manager detection). |
| `check`      | Verifies that all required dependencies (like git) are installed.                                                                                          |
| `sync`       | Clones or updates the package database, and updates the local system configuration by detecting available package managers.                                |
| `upgrade`    | Downloads the latest release of Zoi and replaces the current executable.                                                                                   |
| `clean`      | Clears the cache of downloaded package binaries.                                                                                                           |
| `autoremove` | Removes packages that were installed as dependencies but are no longer needed.                                                                             |

### Package Management

| Command     | Description                                                                                                                                                                               |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list`      | Lists installed or all available packages. <br/>`--all`: List all packages, not just installed. <br/>`--repo <repo>`: Filter by repository. <br/>`--type <type>`: Filter by package type. |
| `show`      | Shows detailed information about a package. <br/>`--raw`: Display the raw, unformatted package file.                                                                                      |
| `search`    | Searches for a package by name or description. <br/>`--repo <repo>`: Filter by repository. <br/>`--type <type>`: Filter by package type.                                                  |
| `install`   | Installs one or more packages. <br/>`--force`: Force re-installation if the package already exists. <br/>`--interactive`: Choose the installation method interactively.                   |
| `build`     | Builds and installs one or more packages from source. <br/>`--force`: Force the package to be rebuilt.                                                                                    |
| `uninstall` | Removes one or more packages. Also removes any of its dependencies that are no longer needed. For collections, it removes all of its dependencies. |
| `update`    | Updates one or more packages to the latest version.                                                                                                                                       |
| `pin`       | Pins a package to a specific version to prevent updates.                                                                                                                                  |
| `unpin`     | Unpins a package, allowing it to be updated again.                                                                                                                                        |
| `why`       | Explains why a package is installed (e.g. as a dependency or directly).                                                                                                                   |
| `clone`     | Clones the source code repository of one or more packages. A target directory can only be specified when cloning a single package.                                                        |
| `exec`      | Downloads a binary to a temporary cache and runs it without installing it.                                                                                                                |

### Project Environment

| Command | Description                                                                    |
| ------- | ------------------------------------------------------------------------------ |
| `run`   | Executes a command from a local 'zoi.yaml' file. Can be run interactively.     |
| `env`   | Sets up project environments from a 'zoi.yaml' file. Can be run interactively. |

### Service Management

| Command | Description                         |
| ------- | ----------------------------------- |
| `start` | Starts a package that is a service. |
| `stop`  | Stops a running service package.    |

### Repository Management (`repo`)

Manages the list of package repositories that Zoi uses.

| Subcommand    | Description                                                                                                       |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| `repo add`    | Adds a new repository from the available sources or clones a repository from a git URL. Can be run interactively. |
| `repo remove` | Deletes a repository from the active list.                                                                        |
| `repo list`   | Shows all currently active repositories. Use `list all` to see all available repositories and their status.       |

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
# (Optional) An alternate source to resolve this package from. Can be another
# package name (e.g. 'my-app-git'), a URL to a raw .pkg.yaml file, or a local file path.
alt: my-app-v2
# (Optional) The installation method to use for `zoi update`.
# Can be 'binary', 'com_binary', 'script', or 'source'.
updater: binary

# (Optional) A list of binaries this package provides. Used for conflict detection.
bins:
  - my-awesome-app
# (Optional) A list of package names that this package conflicts with.
conflicts:
  - my-awesome-app-legacy

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

- **Check why a package is installed:**
  ```sh
  zoi why <package_name>
  ```
