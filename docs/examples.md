---
title: Package Examples
---

# `pkg.yaml` Examples

This document provides a set of examples for creating `pkg.yaml` files. These files are the core of Zoi's packaging system, defining everything from metadata to installation methods.

## Basic Binary Package

This is the most common type of package. It downloads a pre-compiled binary from a URL and places it in the user's path.

```yaml
# packages/utils/my-cli.pkg.yaml
name: my-cli
version: 1.2.3
description: A simple command-line utility.
website: https://example.com/my-cli
git: https://github.com/user/my-cli
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: MIT

# The 'installation' section defines how to install the package.
# You can have multiple methods, and Zoi will pick the best one.
installation:
  - type: binary # This indicates a direct binary download.
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # Optional: Verify the download against a checksum.
    checksums:
      url: "https://github.com/user/my-cli/releases/download/v{version}/checksums.txt"
```

**Key Fields:**

- `name`, `version`, `description`: Basic package metadata.
- `installation`: A list of methods to install the package.
- `type: binary`: Specifies that Zoi should download the file from the `url` and make it executable.
- `url`: The download link for the binary. Notice the use of placeholders like `{version}` and `{platform}` which Zoi replaces at runtime.
- `platforms`: A list of platforms this installation method supports.
- `checksums`: (Optional but recommended) A way to verify the integrity of the downloaded file. It can be a direct URL to a checksums file or a list of file/checksum pairs.

---

## Compressed Binary Package (`com_binary`)

Sometimes, binaries are distributed within a compressed archive (like `.zip` or `.tar.gz`). The `com_binary` type handles this by extracting the archive and finding the executable.

```yaml
# packages/tools/archiver.pkg.yaml
name: archiver
version: 2.0.0
description: A tool for creating and extracting archives.
website: https://example.com/archiver
git: https://github.com/user/archiver
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: Apache-2.0

installation:
  - type: com_binary # Compressed Binary
    url: "https://github.com/user/archiver/releases/download/v{version}/archiver-v{version}-{platform}.{platformComExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # This map tells Zoi which extension to use for each OS.
    platformComExt:
      linux: "tar.gz"
      macos: "tar.gz"
      windows: "zip"
```

**Key Fields:**

- `type: com_binary`: Tells Zoi to download and extract the file. Zoi will then look for a file inside the archive that matches the package `name`.
- `platformComExt`: A map that defines the file extension for the compressed archive based on the operating system (`linux`, `macos`, `windows`).

---

## Build from Source Package

For packages that need to be compiled on the user's machine, you can use the `source` installation type.

```yaml
# packages/dev/compiler.pkg.yaml
name: compiler
version: 0.1.0
description: A new programming language compiler.
git: https://github.com/user/compiler
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

# Dependencies required to build or run the package.
# Zoi can install dependencies from other package managers.
# The format is `manager:package-name[version]`.
dependencies:
  # Build-time dependencies
  build:
    - zoi:go # Assumes 'go' is a Zoi package.
    - native:make # Assumes 'make' is available via the system's native package manager.
    - cargo:some-build-tool
  # Run-time dependencies
  runtime:
    - native:openssl # A runtime dependency needed by the compiled binary.

installation:
  - type: source
    url: "https://github.com/{git}" # URL to the git repository.
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # Commands to execute in the cloned repository to build and install.
    commands:
      - "make build"
      - "mv ./bin/compiler {store}/compiler" # Move the final binary to the Zoi store.
```

**Key Fields:**

- `dependencies`: A map of dependencies.
  - `build`: A list of packages required to _build_ this package. Zoi will ensure they are installed first.
  - `runtime`: A list of packages required to _run_ this package.
- `type: source`: Indicates that Zoi needs to clone a git repository and run build commands.
- `url`: The URL of the source code repository. `{git}` is a placeholder for the `git` field at the top level.
- `commands`: A list of shell commands to run inside the cloned repository.
- `{store}`: A placeholder for the directory where the final executable should be placed.

---

## Script-Based Package

For installers that provide a shell script (e.g. `install.sh` or `install.ps1`), you can use the `script` installation type. This is common for tools like `nvm` or `rustup`.

```yaml
# packages/tools/dev-env-installer.pkg.yaml
name: dev-env-installer
version: "1.0"
description: An example of a script-based installer.
website: https://example.com/dev-env-installer
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: MIT

installation:
  - type: script
    # The URL to the installation script.
    # Zoi replaces {platformExt} with 'sh' on Linux/macOS and 'ps1' on Windows.
    url: "https://example.com/install.{platformExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
```

**Key Fields:**

- `type: script`: Tells Zoi to download the script from the `url` and execute it.
- `url`: The download link for the script. `{platformExt}` is a placeholder that resolves to the correct script extension for the user's OS.

---

## Package Collection

A `collection` is a meta-package that doesn't install any files itself but groups other packages as dependencies. This is useful for setting up development environments.

```yaml
# packages/collections/rust-dev-tools.pkg.yaml
name: rust-dev-tools
type: collection # Set the package type to 'collection'.
version: "1.0"
description: A collection of essential tools for Rust development.
maintainer:
  name: "Community"
  email: "community@example.com"

# The 'runtime' dependencies are the packages that will be installed.
dependencies:
  runtime:
    - rustup
    - cargo-edit
    - cargo-watch
    - clippy
```

**Key Fields:**

- `type: collection`: Defines this as a collection package.
- `dependencies.runtime`: The list of Zoi packages to install when this collection is installed.

---

## Service Package

A `service` package is for applications that need to run in the background (e.g. databases, web servers). Zoi can manage starting and stopping these services.

```yaml
# packages/services/my-database.pkg.yaml
name: my-database
type: service # Set the package type to 'service'.
version: "5.7"
description: A lightweight database server.
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

installation:
  - type: binary
    url: "https://example.com/my-database-v{version}-{platform}"
    platforms: ["linux-amd64", "macos-amd64"]

# The 'service' section defines how to manage the service.
service:
  - platforms: ["linux-amd64", "macos-amd64"]
    start:
      - "my-database --config /etc/my-database.conf"
    stop:
      - "pkill my-database"
```

**Key Fields:**

- `type: service`: Defines this as a service package.
- `service`: A list of service definitions for different platforms.
- `start`: A list of commands to run to start the service.
- `stop`: A list of commands to run to stop the service.

---

## Configuration Package

A `config` package manages the installation and removal of configuration files. It doesn't install an executable itself but can depend on the application it configures. When installed, Zoi will ask the user if they want to run the setup commands.

```yaml
# packages/configs/my-app-config.pkg.yaml
name: my-app-config
type: config # Set the package type to 'config'.
version: "1.0"
description: "Configuration files for my-app."
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

dependencies:
  runtime:
    - my-app # This config depends on 'my-app' being installed.

# The 'config' section defines how to manage the configuration files.
config:
  - platforms: ["linux-amd64", "macos-amd64"]
    # These commands are run to place the config files.
    # Assume your package repo includes a 'config.toml' file.
    install:
      - "mkdir -p ~/.config/my-app"
      - "cp ./config.toml ~/.config/my-app/config.toml"
    # These commands are run when the user uninstalls the config.
    uninstall:
      - "rm ~/.config/my-app/config.toml"
```

**Key Fields:**

- `type: config`: Defines this as a configuration package.
- `dependencies.runtime`: It's good practice to make the config depend on the application it's for.
- `config`: A list of configuration definitions for different platforms.
- `install`: A list of commands to copy or create the configuration files.
- `uninstall`: (Optional) A list of commands to clean up the configuration files upon uninstallation.
