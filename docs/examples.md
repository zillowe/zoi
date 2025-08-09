---
title: Package Examples
description: Set of packages examples
---

This document provides a set of examples for creating `pkg.yaml` files. These files are the core of Zoi's packaging system, defining everything from metadata to installation methods.

## Basic Binary Package

This is the most common type of package. It downloads a pre-compiled binary from a URL and places it in the user's path.

```yaml
# utils/my-cli.pkg.yaml
name: my-cli
repo: community
version: 1.2.3
description: A simple command-line utility.
website: https://example.com/my-cli
readme: https://example.com/my-cli/README.md
git: https://github.com/user/my-cli
tags: [cli, tools]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
  # URL to the maintainer's public GPG key, or the key's 40-character fingerprint.
  key: "DEADC0DEDEADBEEFDEADC0DEDEADBEEFDEADC0DE"
  # (Optional) Website of the maintainer
  website: "https://maintainer.com"
# If the author is different from the maintainer.
author:
  name: "Original Author"
  # A URL is also valid.
  key: "https://keys.example.com/author.gpg"
license: MIT

# The 'installation' section defines how to install the package.
# You can have multiple methods, and Zoi will pick the best one.
installation:
  - type: binary # This indicates a direct binary download.
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # Optional: Verify the download against a checksum.
    checksums:
      # Option A: simple URL (defaults to sha512)
      url: "https://github.com/user/my-cli/releases/download/v{version}/checksums.txt"
      # Option B: explicit list with algorithm type
      # type: sha256 # or sha512 (default)
      # list:
      #   - file: "my-cli-zip"
      #     checksum: "<hex-digest-or-url>"
    # Optional: Verify the download against a GPG signature.
    sigs:
      - file: "my-cli-{platform}"
        sig: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}.sig"

  - type: com_binary
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-v{version}-{platform}.{platformComExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    platformComExt:
      linux: tar.gz
      macos: tar.gz
      windows: zip
    # The executable lives at this path inside the archive.
    binary_path: "bin/my-cli"
```

**Key Fields:**

- `name`, `version`, `description`: Basic package metadata.
- `maintainer`, `author`: Information about who maintains and created the package. The `key` field should be a URL to a public GPG key used for signature verification.
- `installation`: A list of methods to install the package.
- `type: binary`: Specifies that Zoi should download the file from the `url` and make it executable.
- `url`: The download link for the binary. Notice the use of placeholders like `{version}` and `{platform}` which Zoi replaces at runtime.
- `platforms`: A list of platforms this installation method supports.
- `checksums`: (Optional but recommended) A way to verify the integrity of the downloaded file. It can be a direct URL to a checksums file or a list of file/checksum pairs.
- `sigs`: (Optional but highly recommended) A list defining GPG signatures for downloaded files. Zoi uses the `key` from the `maintainer` or `author` to verify the signature.

---

## Compressed Binary Package

Sometimes, binaries are distributed within a compressed archive (like `.zip` or `.tar.gz`). The `com_binary` type handles this by extracting the archive and finding the executable.

```yaml
# tools/archiver.pkg.yaml
name: archiver
repo: community
version: 2.0.0
description: A tool for creating and extracting archives.
website: https://example.com/archiver
git: https://github.com/user/archiver
tags:
  - cli
  - archiver
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
# dev/compiler.pkg.yaml
name: compiler
repo: community
version: 0.1.0
description: A new programming language compiler.
git: https://github.com/user/compiler
tags: [compiler, devtools]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

dependencies:
  build:
    required:
      - zoi:go
      - native:make
    optional:
      - cargo:some-build-tool:additional build helper
  runtime:
    required:
      - native:openssl

installation:
  - type: source
    url: "https://github.com/{git}" # URL to the git repository.
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # Commands to execute in the cloned repository to build and install.
    # Optionally pick a tag or branch (only one). {version} will be expanded.
    # tag: "v{version}"
    # branch: "release/{version}"
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
# tools/dev-env-installer.pkg.yaml
name: dev-env-installer
repo: community
version: "1.0"
description: An example of a script-based installer.
website: https://example.com/dev-env-installer
tags: [installer, env]
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
# collections/web-dev-essentials.pkg.yaml
name: web-dev-essentials
repo: community
type: collection # Set the package type to 'collection'.
version: "1.0"
description: A collection of essential tools for web development.
tags: [collection, web, devtools]
maintainer:
  name: "Community"
  email: "community@example.com"

# The 'runtime' dependencies are the packages that will be installed.
# This collection demonstrates pulling tools from different package managers.
dependencies:
  runtime:
    required:
      - zoi:node
      - zoi:bun
      - native:git
    optional:
      - npm:pnpm
      - npm:serve
      - npm:typescript
```

**Key Fields:**

- `type: collection`: Defines this as a collection package.
- `dependencies.runtime`: The list of packages to install when this collection is installed. Note the `manager:package` format.

---

## Service Package

A `service` package is for applications that need to run in the background (e.g. databases, web servers). Zoi can manage starting and stopping these services.

```yaml
# services/my-database.pkg.yaml
name: my-database
repo: community
type: service # Set the package type to 'service'.
version: "5.7"
description: A lightweight database server.
tags: [service, database]
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
# configs/my-app-config.pkg.yaml
name: my-app-config
repo: community
type: config # Set the package type to 'config'.
version: "1.0"
description: "Configuration files for my-app."
tags: [config]
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

---

## Package with Optional Dependencies

You can define optional dependencies that the user will be prompted to install. This is useful for adding extra functionality without bloating the default installation.

```yaml
# dev/my-dev-tool.pkg.yaml
name: my-dev-tool
repo: community
version: 3.0.0
description: A developer tool with optional integrations.
git: https://github.com/user/my-dev-tool
tags:
  - cli
  - devtools
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

dependencies:
  build:
    required:
      - native:make
      - native:gcc
    optional:
      - native:rust:for rust language support
      - native:go:for go language support
  runtime:
    required:
      - zoi:some-base-library
    optional:
      - zoi:plugin-A:adds feature X
      - zoi:plugin-B:adds feature Y
```

**Key Fields:**

- **required**: Always installed dependencies.
- **optional**: Prompted dependencies; use `manager:package:description` for inline descriptions.
- **options groups**: Under `required.options`, define selectable providers with `name`, `desc`, `all`, and `depends` where each item may have an inline description.

---

## Package with Selectable Required Dependencies

For packages that can work with different backends or libraries, you can let the user choose which one to install. This is handled by structuring the `required` dependencies with an `options` block.

```yaml
# gui/my-cross-platform-app.pkg.yaml
name: my-cross-platform-app
repo: community
version: 1.0.0
description: An application that supports multiple GUI toolkits.
tags: [gui]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

dependencies:
  runtime:
    required:
      # A simple, non-optional required dependency.
      - zoi:core-utils
      # A group of selectable dependencies.
      options:
        - name: "GUI Toolkit"
          desc: "Choose a GUI provider for the application"
          all: false # The user must pick only one of these.
          depends:
            - native:qt6:Recommended for KDE Plasma
            - native:gtk4:Recommended for GNOME
            - native:libadwaita:For a modern GNOME look and feel

  build:
    required:
      - zoi:build-utils
    options:
      - name: "Build GUI Toolkit"
        desc: "Choose GUI dev libraries"
        all: true
        depends:
          - native:qt6-dev:KDE toolkit headers and libs
          - native:gtk4-dev:GNOME toolkit headers and libs
```

**Key Fields:**

- `dependencies.runtime.required.options`: This defines a list of choices for the user.
  - `name`: The name of the choice group (e.g. "GUI Toolkit").
  - `desc`: A description of what the user is choosing.
  - `all`: If `no`, the user can only select one option. If `yes`, they can select multiple (e.g. for installing multiple plugins).
  - `depends`: A list of the actual dependencies the user can choose from. The `manager:package:description` format is used here.

---

## Nested Repository Package

You can organize packages into nested subdirectories within a repository for better organization. To install a package from a nested repository, you must specify the full path.

```yaml
# drivers/nvidia.pkg.yaml
# This file would be located at: Zoi-Pkgs/core/linux/amd64/nvidia.pkg.yaml
name: nvidia-driver
repo: core/linux/amd64
version: "550.78"
description: "NVIDIA driver for Linux."
website: https://www.nvidia.com/
git: https://github.com/NVIDIA/open-gpu-kernel-modules
tags: [driver, gpu]
maintainer:
  name: "Zoi Community"
  email: "community@example.com"
license: "MIT"

installation:
  - type: binary
    url: "https://us.download.nvidia.com/XFree86/Linux-x86_64/{version}/NVIDIA-Linux-x86_64-{version}.run"
    platforms: ["linux-amd64"]
```

**Key Fields:**

- `repo`: The full path to the nested repository.
- To install this package, you would run: `zoi install @core/linux/amd64/nvidia-driver`.

---

## Package with Post-Install/Uninstall Hooks

You can define platform-specific commands to be run after a successful installation (`post_install`) or before uninstallation (`post_uninstall`). This is useful for tasks like setting up shell completions, running initialization scripts, or cleaning up resources.

Zoi will prompt the user for confirmation before executing these commands.

```yaml
# utils/my-cli-with-hooks.pkg.yaml
name: my-cli-with-hooks
repo: community
version: 1.5.0
description: A CLI tool that sets up its own shell completions.
website: https://example.com/my-cli-with-hooks
git: https://github.com/user/my-cli-with-hooks
tags: [cli]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: MIT

installation:
  - type: binary
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]

# The 'post_install' section defines commands to run after installation.
post_install:
  - platforms: ["linux", "macos"] # For Linux and macOS
    commands:
      - "echo 'To finish setup, run the following command in your shell:'"
      - "echo 'eval \"$({name} completion bash)\"'
  - platforms: ["windows"] # For Windows
    commands:
      - "echo 'Installation of {name} v{version} complete!'"

# The 'post_uninstall' section defines commands to run before uninstallation.
post_uninstall:
  - platforms: ["linux", "macos"]
    commands:
      - "echo 'Note: Shell completions may need to be removed manually.'"
```

**Key Fields:**

- `post_install`: A list of post-installation hooks.
- `post_uninstall`: A list of pre-uninstallation hooks.
- `platforms`: Specifies which platforms the commands apply to.
- `commands`: A list of shell commands to be executed. Placeholders like `{name}` and `{version}` are available.

---

## Package with Custom Updater

The `updater` field allows you to specify which installation method `zoi update` should use. This is useful if you want to provide a binary for initial installation but force a build from source during updates to ensure the user has the latest code.

```yaml
# dev/my-language-server.pkg.yaml
name: my-language-server
repo: community
version: 0.5.0
description: A language server that should be updated from source.
tags: [language-server, devtools]
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

# This tells 'zoi update' to always use the 'source' method.
updater: source

installation:
  # The source method will be used for updates.
  - type: source
    url: "https://github.com/user/my-language-server"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    commands:
      - "cargo build --release"
      - "mv ./target/release/my-language-server {store}/my-language-server"

  # The binary method is a fast option for the first install.
  - type: binary
    url: "https://github.com/user/my-language-server/releases/download/v{version}/my-language-server-{platform}"
    platforms: ["linux-amd64", "macos-amd64"]
```

**Key Fields:**

- `updater: source`: This line instructs Zoi to ignore other installation methods when the user runs `zoi update my-language-server` and instead use the one with `type: source`.

---

## Project `zoi.yaml` Example

Use `zoi.yaml` to define project commands and environment setups that can be executed with `zoi run` and `zoi env`.

```yaml
# zoi.yaml (placed at your project root)
name: my-project

packages:
  - name: git
    check: git --version
  - name: node
    check: node --version

commands:
  - cmd: dev
    run: npm run dev
  - cmd: test
    run: npm test

environments:
  - name: Web build
    cmd: web
    run:
      - npm ci
      - npm run build
  - name: Rust toolchain
    cmd: rust
    run:
      - rustup toolchain install stable
      - rustup component add clippy rustfmt
```

Run with:

```sh
zoi run dev    # execute a command by alias
zoi run        # interactive command chooser
zoi env web    # set up an environment by alias
zoi env        # interactive environment chooser
```

See full schema and best practices in [Project Configuration (zoi.yaml)](./project-config).

---

## Package with Remote Version URL (JSON or Plain Text)

You can point the `version` field (or a channel under `versions`) to a URL. Zoi will fetch the URL and resolve the version.

Accepted responses at the URL:
- Plain text containing a version string, e.g. `1.2.3`
- JSON with either:
  - `{ "versions": { "stable": "1.2.3" } }` (for channel resolution)
  - `{ "latest": { "production": { "tag": "v1.2.3" } } }`

Example layout and package file:

```yaml
# packages/zoi/zoi.pkg.yaml
name: zoi
repo: core
description: Zoi CLI packaged via remote version metadata

# Option A: Direct version URL (plain text or acceptable JSON)
version: https://example.com/app/version.json

# Option B: Channel map pointing to a JSON URL (recommended)
# versions:
#   stable: https://example.com/app/version.json
#   beta: https://example.com/app/beta.json

installation:
  - type: binary
    url: "https://downloads.example.com/zoi-{platform}-{version}"
    platforms: ["linux-amd64", "macos-arm64", "windows-amd64"]

# Alternatively, use a compressed archive
#  - type: com_binary
#    url: "https://downloads.example.com/zoi-{version}-{platform}.{platformComExt}"
#    platforms: ["linux-amd64", "macos-arm64", "windows-amd64"]
#    platformComExt:
#      linux: tar.zst
#      macos: tar.zst
#      windows: zip
```

Example `app/version.json` payloads that Zoi understands:

```json
{
  "versions": {
    "stable": "3.8.0",
    "beta": "3.9.0-beta"
  }
}
```

```json
{
  "latest": {
    "production": { "tag": "v3.8.0" }
  }
}
```

If the URL returns plain text, it should be a single version string like `3.8.0`.

## Package with `alt` Redirection

The `alt` field redirects Zoi to install a different package. This is perfect for creating aliases or pointing to a package definition hosted elsewhere. The value can be another package name, a URL to a raw `.pkg.yaml` file, or a local file path.

### Example 1: Alias to Another Package

This is useful for creating a simpler name for a package.

First, the actual binary package, `my-app-bin.pkg.yaml`:

```yaml
# utils/my-app-bin.pkg.yaml
name: my-app-bin
repo: community
version: 1.0.0
description: The actual binary for my-app.
# ... (rest of the installation details)
```

Second, the alias package, `my-app.pkg.yaml`:

```yaml
# utils/my-app.pkg.yaml
name: my-app
repo: community
version: 1.0.0
description: A friendly alias for my-app-bin.
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

# This tells Zoi to install 'my-app-bin' instead of this package.
alt: my-app-bin
```

### Example 2: Redirecting to a URL

You can also use `alt` to point to a package file hosted on a different server or in a gist. This is useful for testing or for packages that are not in an official repository.

```yaml
# utils/my-remote-app.pkg.yaml
name: my-remote-app
repo: community
version: 1.0.0
description: An alias for a package hosted on a remote server.
maintainer:
  name: "Your Name"
  email: "your.email@example.com"

# Zoi will download and install the package from this URL.
alt: https://example.com/my-app.pkg.yaml
```

**Key Fields:**

- `alt`: When a user runs `zoi install my-app` or `zoi install my-remote-app`, Zoi sees this field, stops processing the current file, and immediately starts resolving the source specified in `alt`.

---

## Package with Conflict Detection

You can define `bins` and `conflicts` to help Zoi prevent conflicts between packages. Zoi will warn the user if an installation would cause a conflict and ask for confirmation.

```yaml
# utils/my-cli-v2.pkg.yaml
name: my-cli-v2
repo: community
version: 2.0.0
description: A new version of my-cli that conflicts with the old one.
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: MIT

# This package provides two binaries: 'my-cli' and 'mcli'.
# If another installed package also provides 'my-cli', Zoi will warn you.
bins:
  - my-cli
  - mcli

# This package explicitly conflicts with the 'my-cli-legacy' package.
# If 'my-cli-legacy' is installed, Zoi will warn you.
conflicts:
  - my-cli-legacy

installation:
  - type: binary
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}"
    platforms: ["linux-amd64", "macos-amd64"]
```

**Key Fields:**

- `bins`: A list of the executable files the package provides. Zoi checks if any of these binaries are already provided by another installed package.
- `conflicts`: A list of other Zoi package names that are incompatible with this one.
