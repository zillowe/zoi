---
title: Creating & Publishing Packages
description: A complete guide on how to create and publish a package for Zoi.
---

This guide provides a start-to-finish walkthrough of creating a new package, testing it locally, and publishing it to the official Zoi package repositories for everyone to use.

## Understanding Zoi Repositories

Zoi organizes its packages into several repositories, each with a specific purpose. When you contribute a new package, you'll need to decide which repository is the best fit.

| Repository  | Description                                                                                                                             |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `core`      | Contains essential, fundamental packages that are often dependencies for other tools (e.g. `vim`, `git`). These are tightly maintained. |
| `main`      | A curated set of popular, high-quality, and well-maintained packages that are useful for a wide audience (e.g. `node`, `go`).           |
| `extra`     | A broader collection of useful packages that may not be as universally applicable as those in `main`.                                   |
| `community` | The primary repository for packages submitted by the community. This is the best place for new contributions.                           |
| `test`      | Used internally for testing new Zoi features and package builds. Not intended for general use.                                          |
| `archive`   | Contains older or deprecated packages that are no longer maintained but are kept for historical purposes.                               |

For your first contribution, you will almost always be adding your package to the **`community`** repository.

## Step 1: Creating Your `pkg.yaml` File

The heart of every Zoi package is a `pkg.yaml` file. This file contains all the metadata and instructions Zoi needs to install your software.

### Basic Structure

At a minimum, your package needs these fields:

```yaml
# my-cli.pkg.yaml
name: my-cli
repo: community
version: 1.2.3
description: A simple command-line utility.
maintainer:
  name: "Your Name"
  email: "your.email@example.com"
license: MIT
```

- `name`: The unique identifier for your package.
- `version`: The current version of the software.
- `description`: A short, one-sentence summary of what the package does.
- `maintainer`: Your name and email.
- `license`: The software's license (e.g. `MIT`, `GPL-3.0-or-later`).

It's also highly recommended to add:

- `website`: The official project website.
- `git`: The URL of the source code repository.

### Package Types

Zoi supports different types of packages. You can specify the type using the `type` field.

- `package` (Default): A standard software package.
- `collection`: A meta-package that only installs a list of other packages as dependencies.
- `service`: A package that runs as a background service (e.g. a database).
- `config`: A package that manages configuration files for another application.

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
```

#### `source`

For packages that need to be compiled from source code.

```yaml
installation:
  - type: source
    url: "https://github.com/{git}" # {git} is a placeholder for the top-level git field
    platforms: ["linux-amd64", "macos-amd64", "windows-amd64"]
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

## Step 3: Adding Dependencies

If your package requires other tools to be installed, define them in the `dependencies` section.

- `build`: Dependencies needed only to compile the package (for `source` builds).
- `runtime`: Dependencies needed to run the package.
- `required`: Dependencies that are always installed.
- `optional`: Dependencies that the user is prompted to install, useful for plugins or extra features.

The format is `manager:package-name:description` (the description is for optional dependencies).

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

## Step 4: Adding Post-Installation Hooks

Some packages may require additional setup steps after the main installation is complete, such as setting up shell completions or running a configuration wizard. The `post_install` field allows you to define platform-specific commands that run after a successful installation.

Zoi will ask for user confirmation before running these commands for security.

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

## Step 5: Publishing Your Package

Once your package works locally, it's time to share it with the world! This is done by adding your `pkg.yaml` file to the official Zoi packages database.

The Zoi package database is hosted on GitLab and mirrored on GitHub.

- **GitLab (Primary):** [Zillowe/Zillwen/Zusty/Zoi-Pkgs](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi-Pkgs)
- **GitHub (Mirror):** [Zillowe/Zoi-Pkgs](https://github.com/Zillowe/Zoi-Pkgs)

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
