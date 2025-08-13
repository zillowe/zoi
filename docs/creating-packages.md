---
title: Creating & Publishing Packages
description: A complete guide on how to create and publish a package for Zoi.
---

This guide provides a start-to-finish walkthrough of creating a new package, testing it locally, and publishing it to the official Zoi package repositories for everyone to use.

## Understanding Zoi Repositories

Zoi organizes its packages into several repositories, each with a specific purpose. When you contribute a new package, you'll need to decide which repository is the best fit.

| Repository  | Description                                                                       |
| ----------- | --------------------------------------------------------------------------------- |
| `core`      | Essential packages and libraries; very common and well-maintained.                |
| `main`      | Important packages that don't fit in `core` but are essential for most users.     |
| `extra`     | New or niche packages; less common and may be less actively maintained.           |
| `community` | User-submitted packages. New entries start here and may graduate to higher tiers. |
| `test`      | Testing ground for new Zoi features and packages prior to release.                |
| `archive`   | Archived packages that are no longer maintained.                                  |

For your first contribution, you will almost always be adding your package to the **`community`** repository.

For more information about repositories [visit here](/docs/zds/zoi/repositories/)

## Step 1: Creating Your `pkg.yaml` File

The heart of every Zoi package is a `pkg.yaml` file. This file contains all the metadata and instructions Zoi needs to install your software.

You can create this file manually, or use Zoi's interactive helper.

### Option A: Using the Interactive `make` Command (Recommended)

The easiest way to create a valid package file is with the `zoi make` command. It launches an interactive terminal UI that guides you through filling out all the necessary fields.

```sh
# Launch the interactive creator
zoi make

# You can also pre-fill the package name
zoi make my-new-app
```

This will create a `my-new-app.pkg.yaml` file in your current directory with all the correct formatting and even the JSON schema link for editor autocompletion.

### Option B: Creating the File Manually

If you prefer to create the file by hand, here's what you need to know.

#### Using the JSON Schema for Validation

To help you create valid `pkg.yaml` files and get autocompletion in supported editors (like VS Code), you can add a `$schema` tag pointing to the official Zoi package schema.

```yaml
# my-cli.pkg.yaml
# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json
name: my-cli
repo: community
# ... rest of your package definition
```

This line tells your editor to use the schema for validation, which can catch errors before you even test the package.

### Basic Structure

At a minimum, your package needs these fields:

```yaml
# my-cli.pkg.yaml
# yaml-language-server: $schema=https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/raw/main/app/pkg.schema.json
name: my-cli
repo: community
version: 1.2.3
description: A simple command-line utility.
website: https://example.com/my-cli
readme: https://example.com/my-cli/README.md
git: https://github.com/user/my-cli
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
  # Optional: URL to your public GPG key
  key: "https://keys.example.com/your-key.gpg"
  # (Optional) Website of the maintainer
  website: "https://maintainer.com"
# Optional: if the package author is different
author:
  name: "Original Author"
  key: "https://keys.example.com/author-key.gpg"
license: MIT
```

- `name`: The unique identifier for your package.
- `version`: The current version of the software. You can also set this to `"{git}"` to automatically use the latest stable release tag from the GitHub or GitLab repository specified in the `git` field.
- `description`: A short, one-sentence summary of what the package does.
- `maintainer`: Your name, website and email.
- `license`: The software's license (e.g. `MIT`, `GPL-3.0-or-later`).

It's also highly recommended to add:

- `website`: The official project website.
- `git`: The URL of the source code repository.
- `readme`: A URL to a README file (markdown or plain text).

### Package Types

Zoi supports different types of packages. You can specify the type using the `type` field.

- `package` (Default): A standard software package.
- `collection`: A meta-package that only installs a list of other packages as dependencies.
- `service`: A package that runs as a background service (e.g. a database).
- `config`: A package that manages configuration files for another application.
- `app`: An application template used with `zoi create <source> <appName>` to scaffold projects. Not installable directly.

### Tags (Recommended)

Add a `tags` list to improve discoverability in `zoi search` and to let users filter by tags.

```yaml
tags:
  - cli
  - devtools
  - editor
```

## Step 2: Defining an Installation Method

The `installation` section tells Zoi how to get the software onto a user's machine. You can provide multiple methods, and Zoi will pick the best one for the user's platform.

#### `binary`

For downloading a single, pre-compiled executable.

```yaml
installation:
  - type: binary
    url: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
```

#### `com_binary` (Compressed Binary)

For downloading a `.zip` or `.tar.gz` archive that contains the binary.

```yaml
installation:
  - type: com_binary
    url: "https://github.com/user/tool/releases/download/v{version}/tool-v{version}-{platform}.{platformComExt}"
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    platformComExt:
      linux: "tar.gz"
      macos: "tar.gz"
      windows: "zip"
    # (Optional) Path or filename of the executable inside the archive.
    # If this ends with .exe, Zoi installs it as <package>.exe; otherwise <package>.
    # Can be a relative path (bin/tool) or just a filename (tool.exe).
    # On Windows targets, if this does not end with .exe, Zoi will also try with .exe appended.
    binary_path: "bin/tool"
```

Supported archive formats for `com_binary` are documented in [Supported Archives for Compressed Binaries](/docs/zds/zoi/archives/).

#### `source`

For packages that need to be compiled from source code.

```yaml
installation:
  - type: source
    url: "https://github.com/{git}" # {git} is a placeholder for the top-level git field
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
    # (Optional) One of these may be specified to pick a ref
    # tag can include placeholders like v{version}
    # tag: "v{version}"
    # branch: "release/{version}"
    commands:
      - "make build"
      - "mv ./bin/compiler {store}/compiler" # {store} is the path to Zoi's install directory
```

#### `script`

For tools that use an installation script (e.g. `install.sh`).

```yaml
installation:
  - type: script
    url: "https://example.com/install.{platformExt}" # {platformExt} becomes 'sh' or 'ps1'
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
```

### Placeholders

Zoi uses placeholders to make your URLs dynamic:

- `{version}`: The package version.
- `{platform}`: The user's platform (e.g. `linux-amd64`).
- `{platformComExt}`: The correct compressed archive extension for the OS.
- `{platformExt}`: The correct script extension for the OS.
- `{git}`: The value of the top-level `git` field.
- `{store}`: The path where the final binary should be placed (for `source` builds).

For resolving a package version from a remote URL (plain text or JSON), see the example in [Package with Remote Version URL](/docs/zds/zoi/examples/#package-with-remote-version-url-json-or-plain-text).

### Security: Checksums

It is **highly recommended** to include checksums to verify the integrity of downloaded files.

```yaml
installation:
  - type: binary
    url: "..."
    platforms: ["..."]
     checksums:
       # Option 1: URL to a checksums file (e.g. checksums.txt)
       # Defaults to sha512 algorithm
       url: "https://github.com/user/my-cli/releases/download/v{version}/checksums.txt"
       # Option 2: Explicit list with algorithm type
       # type: sha256 # or sha512 (default)
       # list:
       #   - file: "my-cli-zip"
       #     # Hex digest or URL to a file containing the digest
       #     checksum: "<hex-digest-or-url>"
```

### Security: Signatures

For an even higher level of security, you can add GPG signature verification. This ensures that the package was published by a trusted developer.

This requires two parts:

1.  **Provide a GPG Key:** Add a `key` field to the `maintainer` or `author` sections. The value can be a URL pointing to the public GPG key, or the key's 40-character fingerprint. Zoi will fetch fingerprints from `keys.openpgp.org`.

    ```yaml
    maintainer:
      name: "Your Name"
      email: "your.email@example.com"
      # Can be a URL to the key, or the key's fingerprint
      key: "DEADC0DEDEADBEEFDEADC0DEDEADBEEFDEADC0DE"

    author:
      name: "Original Author"
      # A URL is also valid
      key: "https://keys.example.com/author-key.gpg"
    ```

2.  **Add Signature Information:** In the `installation` method, add a `sigs` list. Each item specifies a file and a URL to its corresponding `.sig` file.

    ```yaml
    installation:
      - type: binary
        url: "..."
        platforms: ["..."]
        # ... checksums ...
        sigs:
          - file: "my-cli-{platform}" # The file to verify
            sig: "https://github.com/user/my-cli/releases/download/v{version}/my-cli-{platform}.sig" # URL to the signature
    ```

Zoi will download the key from the URL, import it, and use it to verify the signature of the downloaded file.

## Step 3: Adding Dependencies

If your package requires other tools to be installed, define them in the `dependencies` section. Dependencies are split between `build` (needed to compile from source) and `runtime` (needed to run the application).

Both `build` and `runtime` dependencies can be further divided into `required` and `optional`.

```yaml
dependencies:
  build:
    required:
      - native:make
      - native:gcc
  runtime:
    required:
      - zoi:some-base-library
    optional:
      - zoi:plugin-A:adds feature X
      - zoi:plugin-B:adds feature Y
```

- `required`: Dependencies that are always installed.
- `optional`: Dependencies that the user is prompted to install. The format is `manager:package-name:description`, where the description explains what the dependency provides.

### Selectable Required Dependencies

For more complex scenarios, you can offer the user a choice between different providers for a required dependency. This is useful when your application supports multiple backends (e.g. different GUI toolkits or database drivers).

You can structure the `required` section with an `options` block:

```yaml
dependencies:
  runtime:
    required:
      # You can mix simple required dependencies...
      - zoi:core-library
      # ...with selectable options.
      options:
        - name: "GUI"
          desc: "GUI Providers"
          all: no # 'no' means the user must choose only one. 'yes' allows multiple selections.
          depends:
            - native:qt6:for KDE desktop environments
            - native:gtk4:for GNOME-based desktop environments
```

When a user installs this package, Zoi will prompt them to choose a GUI provider, ensuring the necessary dependency is met while giving the user control.

### Advanced: Full dependency schema

Both `runtime` and `build` support the same structure: a simple list, or an advanced object with `required`, `options`, and `optional` all at once.

```yaml
dependencies:
  runtime:
    required:
      - zoi:core-utils
    options:
      - name: "GUI Toolkit"
        desc: "Choose a GUI provider for the application"
        all: false
        depends:
          - native:qt6:Recommended for KDE Plasma
          - native:gtk4:Recommended for GNOME
          - native:libadwaita:For a modern GNOME look and feel
    optional:
      - zoi:extra-utils:handy extras
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
    optional:
      - zoi:extra-build-utils:extra helpers
```

- Required entries cannot have inline descriptions. If you need description and choice, use an `options` group.
- Optional entries can have inline descriptions using `manager:package:description`.
- In an `options` group, each `depends` item can include an inline description after the second `:`.
- During installation:
  - For normal packages, `build` dependencies are installed when a source build is selected/required (no compatible `binary`/`com_binary`), or when forcing source.
  - For `collection` and `config` packages, both `runtime` and `build` dependencies are honored directly.

## Step 4: Adding Post-Installation & Uninstallation Hooks

Some packages may require additional setup steps after installation is complete, or cleanup steps during uninstallation. The `post_install` and `post_uninstall` fields allow you to define platform-specific commands.

Zoi will ask for user confirmation before running these commands for security.

### `post_install`

These commands run after a successful installation. This is useful for setting up shell completions or running a configuration wizard.

```yaml
post_install:
  - platforms: ["linux", "macos"]
    commands:
      - "echo 'Heads up! {name} needs to do some post-install setup.'"
      - "{name} --setup-completions"
  - platforms: ["windows"]
    commands:
      - "echo 'Successfully installed {name} v{version}!'"
```

- `platforms`: A list of platforms where these commands should run (e.g. `linux`, `macos`, `windows`, `linux-amd64`).
- `commands`: A list of shell commands to execute. You can use the `{name}` and `{version}` placeholders.

### `post_uninstall`

These commands run before a package is uninstalled. This is useful for cleaning up configuration files, removing system services, or deregistering components.

```yaml
post_uninstall:
  - platforms: ["linux", "macos"]
    commands:
      - "echo 'Running cleanup tasks for {name}...'"
      - "{name} --remove-completions"
```

The structure is identical to `post_install`.

## Advanced `pkg.yaml` Features

Beyond the basics, `pkg.yaml` offers powerful fields for more complex scenarios like package aliasing and defining specific update behaviors.

### The `alt` Field: Aliasing and Redirection

The `alt` field allows you to redirect Zoi to resolve a different package source. This is incredibly useful for creating aliases or maintaining multiple versions of a package. When a user tries to install the current package, Zoi will instead fetch and install the one specified in the `alt` field.

The value of `alt` can be:

- Another package name (e.g. `my-app-git`).
- A URL to a raw `.pkg.yaml` file.
- A local file path to a `.pkg.yaml` file.

**Use Case: Creating a "latest" alias**

Imagine you have a package `my-app` that is built from source, but you also want to provide a pre-compiled version for users who don't want to build it. You can create two package files:

1.  `my-app-bin.pkg.yaml`: Installs the pre-compiled binary.
2.  `my-app.pkg.yaml`: An alias that points to the binary version.

```yaml
# my-app.pkg.yaml
name: my-app
repo: community
version: 1.0.0
description: A simple command-line utility (alias for binary).
# ... other metadata ...
alt: my-app-bin # Redirects to the 'my-app-bin' package
```

Now, when a user runs `zoi install my-app`, Zoi will automatically resolve and install `my-app-bin` instead.

### The `updater` Field: Custom Update Logic

The `updater` field gives you control over how `zoi update` behaves for your package. By default, Zoi will try to find a `binary` or `com_binary` to perform an update. However, you can force it to use a different method.

The value of `updater` can be one of the installation types: `binary`, `com_binary`, `script`, or `source`.

**Use Case: Forcing a source build on update**

If your package must always be compiled from the latest source code to function correctly, you can ensure that `zoi update` always pulls and rebuilds it.

```yaml
# my-compiler.pkg.yaml
name: my-compiler
repo: community
version: 0.2.0
description: A compiler that needs to be built from the latest source.
# ...
updater: source # Force 'zoi update' to use the 'source' method

installation:
  - type: source
    url: "https://github.com/{git}"
    commands:
      - "make clean"
      - "make build"
      - "mv ./bin/compiler {store}/compiler"
  - type: binary
    # This binary is provided as an initial install option,
    # but updates will always use the source method above.
    url: "https://github.com/user/compiler/releases/download/v{version}/compiler-{platform}"
    platforms: ["linux-amd64", "macos-amd64"]
```

In this example, a user can get a pre-compiled binary on their first `zoi install`, but every subsequent `zoi update my-compiler` will trigger a fresh build from the source repository, ensuring they always have the latest version.

### Handling Conflicts: `bins` and `conflicts`

To prevent issues where two different packages provide the same command-line tool or are otherwise incompatible, Zoi offers two fields to manage conflicts. If a conflict is detected, Zoi will warn the user and ask for confirmation before proceeding with the installation.

If these fields are not present, Zoi falls back to its default behavior of checking if a command with the same name as the package already exists on the system.

#### The `conflicts` Field

This field lets you declare that your package is incompatible with a list of other Zoi packages.

```yaml
# my-new-editor.pkg.yaml
name: my-new-editor
version: 1.0.0
# ...
conflicts:
  - old-editor # This package cannot be installed if 'old-editor' is present
  - another-editor
```

#### The `bins` Field

This field lists the executable files (binaries) that your package installs. Zoi uses this to detect if another installed package provides a binary with the same name.

```yaml
# my-utils.pkg.yaml
name: my-utils
version: 1.0.0
# ...
# This package installs two commands: 'mu' and 'mu-helper'
bins:
  - mu
  - mu-helper
```

If a user tries to install `my-utils` while another package that also provides a `mu` binary is already installed, Zoi will detect the conflict. This is different from the package name; for example, the `vim` package might provide the `vi` binary, which could conflict with a separate `vi` package.

## Step 5: Testing Your Package Locally

Before you publish your package, you **must** test it locally to ensure it installs correctly.

1.  Save your `my-package.pkg.yaml` file somewhere on your machine.
2.  Run the install command, pointing to your local file:

    ```sh
    zoi install ./path/to/my-package.pkg.yaml
    ```

    If you are testing a `source` build, use the `build` command:

    ```sh
    zoi build ./path/to/my-package.pkg.yaml
    ```

3.  Zoi will attempt to install it just like a user would. Watch for any errors in the output.
4.  After a successful installation, try running the command to make sure it works.
5.  Finally, uninstall it to ensure a clean removal:
    ```sh
    zoi uninstall my-package
    ```

## Step 6: Publishing Your Package

Once your package works locally, it's time to share it with the world! This is done by adding your `pkg.yaml` file to the official Zoi packages database.

The Zoi package database is hosted on GitLab and mirrored on GitHub and Codeberg.

- **GitLab (Primary):** [Zillowe/Zillwen/Zusty/Zoi-Pkgs](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs)
- **GitHub (Mirror):** [Zillowe/Zoi-Pkgs](https://github.com/Zillowe/Zoi-Pkgs)
- **Codeberg (Mirror):** [Zillowe/Zoi-Pkgs](https://codeberg.org/Zillowe/Zoi-Pkgs)

You can contribute by opening a **Merge/Pull Request** to either repository, or by **opening an issue** to request a new package. The following steps outline the process for creating a Merge Request on GitLab, which is very similar to the process on GitHub.

1.  **Fork the Repository:**
    Go to the repository's page on GitLab or GitHub and click the "Fork" button to create your own copy.

2.  **Clone Your Fork:**
    Clone the repository to your local machine.

    ```sh
    # For GitLab
    git clone https://gitlab.com/YourUsername/Zoi-pkgs.git
    cd Zoi-pkgs
    ```

3.  **Choose the Right Directory:**
    As discussed in the first section, you should almost always add new packages to the `community` directory.

    You can also create nested directories to better organize packages. For example, you could place a Linux-specific editor in `community/editors/linux/my-editor.pkg.yaml`. The `repo` field in your package file should then be `community/editors/linux`.

4.  **Add Your Package File:**
    Copy your `my-package.pkg.yaml` file into the `community/` directory.

    ```sh
    cp /path/to/my-package.pkg.yaml community/
    ```

    For a nested repository, create the directory structure and place your file inside:

    ```sh
    mkdir -p community/editors/linux
    cp /path/to/my-editor.pkg.yaml community/editors/linux/
    ```

5.  **Commit and Push:**
    Commit your new package file to your forked repository.

    ```sh
    git add community/my-package.pkg.yaml
    git commit -m "feat(community): add my-package v1.2.3"
    git push origin main
    ```

    For a nested package, your commit might look like this:

    ```sh
    git add community/editors/linux/my-editor.pkg.yaml
    git commit -m "feat(community): add my-editor v1.0.0"
    git push origin main
    ```

6.  **Open a Merge/Pull Request:**
    Go to your forked repository on GitLab or GitHub. You should see a button to "Create merge request" or "Create pull request". Click it.
    - **Title:** Use a conventional commit message like `feat(community): add my-package`.
    - **Description:** Briefly describe what your package does and link to its official website or source code.
    - Submit the request.

A Zoi maintainer will review your submission. They may suggest some changes. Once approved, it will be merged, and your package will be available to everyone after the next `zoi sync`!

## Creating Your Own Git-Based Package Repository

While contributing to the official repositories is great for public packages, you might want to manage your own set of packages for private projects, company-internal tools, or personal use. Zoi makes this easy by allowing you to add any git repository as a package source.

### Step 1: Create Your Git Repository

1.  Create a new, empty repository on a service like GitLab or GitHub.
2.  Add your `*.pkg.yaml` files to the root of the repository. The structure is simple: just a flat collection of package files.

    ```
    my-zoi-repo/
    ├── my-first-package.pkg.yaml
    └── my-second-package.pkg.yaml
    ```

3.  Commit and push your files to the remote repository.

### Step 2: Add Your Repository to Zoi

Use the `zoi repo add` command with your repository's HTTPS or SSH URL. Zoi will clone it locally.

```sh
zoi repo add https://github.com/YourUsername/my-zoi-repo.git
```

Zoi clones the repo into `~/.zoi/pkgs/git/`. The name of the repository is determined from the URL (e.g. `my-zoi-repo`).

### Step 3: Install Packages from Your Repository

To install a package from your custom git repository, use the `@git/` prefix, followed by the repository name and the package name.

```sh
# To install my-first-package from the example above
zoi install @git/my-zoi-repo/my-first-package
```

This allows you to maintain and version your own collections of packages completely independently from the official Zoi databases.
