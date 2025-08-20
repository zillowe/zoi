---
title: Dependencies & Supported Package Managers
description: How Zoi installs dependencies from external package managers and the full set of supported managers.
---

Zoi can install dependencies via many ecosystem and OS package managers. This page documents:

- How to declare dependencies in `pkg.yaml`
- All supported managers, their target platforms/ecosystems, and example usage
- Notes and caveats for specific managers

## Declaring dependencies in `pkg.yaml`

Dependencies are specified in the `dependencies` section. Both `runtime` and `build` dependency groups can be defined as a simple list of required dependencies, or as a more advanced object with `required`, `optional`, and selectable `options`.

### Dependency String Format

Each entry follows the format: `manager:package` with an optional version and an optional inline description.

- **Format:** `manager:package[@version][:description]`
- **Version:** Can be specified with `@<semver>` or a comparator string like `=1.2.3`, `>=2.0.0`, `^1.2`, `~1.2.3`. Do not prefix with `v`.
  - Examples: `npm:typescript@5.3.2`, `cargo:bat@0.24.0`, `apt:curl=7.68.0-1ubuntu2.18`.
  - For `zoi` packages, you can use a channel name like `zoi:my-pkg@stable`.
- **Description:** An inline description can be added after the last colon. It must not contain version characters.
  - Example: `pipx:black:Python formatter CLI`.
- **Native Packages:** Use `native:<pkg>` to have Zoi automatically select the system's package manager.

### Simple Dependency List

For packages with only required dependencies, you can use a simple list.

```yaml
dependencies:
  runtime:
    - native:openssl
    - npm:typescript
    - zoi:some-zoi-package
  build:
    - native:make
```

### Advanced Dependency Object

For more complex scenarios involving optional or selectable dependencies, use the object format.

```yaml
dependencies:
  runtime:
    required:
      - zoi:core-utils
    options:
      - name: "GUI Toolkit"
        desc: "Choose a GUI provider for the application"
        all: false # 'false' means the user must choose one. 'true' allows multiple selections.
        depends:
          - native:qt6:Recommended for KDE Plasma
          - native:gtk4:Recommended for GNOME
    optional:
      - zoi:extra-utils:handy extras
  build:
    required:
      - zoi:build-utils
```

- `required`: A list of dependencies that are always installed.
- `options`: A list of groups where the user is prompted to choose one or more dependencies.
- `optional`: A list of dependencies that the user is prompted to install.

### Templating and Versioning

- **`{version}` Placeholder:** You can reference the parent package's version using `{version}` in a dependency string. It will be replaced before parsing. Example: `zoi:my-plugin@{version}`.
- **`versions` Map:** If your package defines a `versions` map (e.g. `stable: 1.4.3`), Zoi resolves the concrete version and substitutes it for `{version}` in all dependency strings. This is useful for keeping dependency versions in lockstep.

Example with templating:

```yaml
name: my-app
versions:
  stable: 1.4.3
  beta: 1.5.0-beta.2
dependencies:
  runtime:
    required:
      - zoi:my-plugin@{version}
      - npm:my-lib@{version}
```

### Version Pinning Notes

- Zoi parses versions using SemVer requirements. If a manager cannot enforce versions directly, Zoi attempts best-effort checks or warns that pinning may not be honored.
- Some managers support pinning (e.g. `apt` via `pkg=ver`, `dnf` via `pkg-ver`, `choco` via `--version`, `cargo` via `--version`, `npm/yarn/pnpm/bun` via `pkg@ver`, `brew` when a formula tap provides `pkg@ver`).
- Many OS managers do not support explicit pinning in a reliable way (e.g. `pacman`, `yay/paru`, `apk`, `xbps`, `eopkg`, `guix`, `portage`, `snap`, `flatpak`, `macports`, `conda`). Zoi will install the latest available and may print a warning.
- Go modules: Zoi currently installs with `go install <module>@latest`. Declaring a version like `go:module@...` is not supported through Zoi's SemVer parser and may fail.

## Supported managers

Below is a comprehensive list of all supported dependency managers, their target platforms or ecosystems, and an example of how to use them in your `pkg.yaml`.

| Manager          | Platform / Ecosystem | Example Usage                         |
| ---------------- | -------------------- | ------------------------------------- |
| `zoi`            | Zoi (all)            | `zoi:my-package`                      |
| `native`         | System (auto)        | `native:openssl`                      |
| `script`         | URL (all)            | `script:example.com/install`          |
| `apt`, `apt-get` | Debian/Ubuntu        | `apt:build-essential`                 |
| `pacman`         | Arch Linux           | `pacman:base-devel`                   |
| `yay`            | Arch Linux (AUR)     | `yay:google-chrome`                   |
| `paru`           | Arch Linux (AUR)     | `paru:visual-studio-code-bin`         |
| `pikaur`         | Arch Linux (AUR)     | `pikaur:spotify`                      |
| `trizen`         | Arch Linux (AUR)     | `trizen:zoom`                         |
| `aur`            | Arch Linux (AUR)     | `aur:slack-desktop`                   |
| `dnf`, `yum`     | Fedora/RHEL          | `dnf:libX11-devel`                    |
| `zypper`         | openSUSE             | `zypper:libopenssl-devel`             |
| `apk`            | Alpine Linux         | `apk:build-base`                      |
| `portage`        | Gentoo               | `portage:dev-libs/openssl`            |
| `xbps-install`   | Void Linux           | `xbps-install:base-devel`             |
| `eopkg`          | Solus                | `eopkg:system.devel`                  |
| `guix`           | GNU Guix             | `guix:gcc-toolchain`                  |
| `brew`           | macOS (Homebrew)     | `brew:node`                           |
| `brew-cask`      | macOS (Homebrew)     | `brew-cask:visual-studio-code`        |
| `mas`            | macOS App Store      | `mas:1295203466`                      |
| `macports`       | macOS (MacPorts)     | `macports:openssl`                    |
| `scoop`          | Windows              | `scoop:git`                           |
| `choco`          | Windows (Chocolatey) | `choco:git`                           |
| `winget`         | Windows              | `winget:Git.Git`                      |
| `snap`           | Linux (Snapcraft)    | `snap:node`                           |
| `flatpak`        | Linux (Flathub)      | `flatpak:org.gimp.GIMP`               |
| `pkg`            | FreeBSD              | `pkg:git`                             |
| `pkg_add`        | OpenBSD              | `pkg_add:git`                         |
| `cargo`          | Rust                 | `cargo:ripgrep`                       |
| `cargo-binstall` | Rust (binary)        | `cargo-binstall:ripgrep`              |
| `go`             | Go                   | `go:golang.org/x/tools/cmd/goimports` |
| `npm`            | Node.js              | `npm:typescript`                      |
| `yarn`           | Node.js              | `yarn:prettier`                       |
| `pnpm`           | Node.js              | `pnpm:eslint`                         |
| `bun`            | Bun                  | `bun:elysia`                          |
| `volta`          | JavaScript           | `volta:node`                          |
| `deno`           | Deno                 | `deno:npm-chalk`                      |
| `jsr`            | JavaScript Registry  | `jsr:@std/http`                       |
| `pip`            | Python               | `pip:requests`                        |
| `pipx`           | Python               | `pipx:black`                          |
| `uv`             | Python               | `uv:ruff`                             |
| `conda`          | Conda                | `conda:numpy`                         |
| `gem`            | Ruby                 | `gem:rails`                           |
| `composer`       | PHP                  | `composer:laravel/installer`          |
| `dotnet`         | .NET                 | `dotnet:fantomas`                     |
| `nix`            | Nix                  | `nix:nixpkgs.hello`                   |
| `dart-pub`       | Dart                 | `dart-pub:shelf`                      |

Notes:

- AUR: `aur:<pkg>` builds from source using `makepkg`; uninstall is done with `pacman`.
- `native:<pkg>` selects the appropriate system manager based on OS/distro; if none can be detected, Zoi errors.
- Some managers (e.g. `go`, `jsr`, `volta`) do not provide reliable uninstall; Zoi prints a notice and skips.
- The `script` manager takes a URL as the package name (e.g. `script:example.com/install`). It appends `.sh` for Linux/macOS and `.ps1` for Windows, then downloads and executes the script. There is no automatic uninstallation.

## Zoi Dependencies and Conflict Checks

When a dependency uses the `zoi:` manager, Zoi resolves the referenced package definition and applies the same conflict checks as for a top-level install:

- If the dependency package declares `bins`, Zoi checks whether any of those binaries are already provided by installed packages.
- If the dependency package declares `conflicts`, Zoi checks whether any listed packages are installed.

If conflicts are detected, Zoi displays the conflicts and prompts whether to continue before proceeding with installation.

## Reproducible Installs with Sharable Manifests

When you install a package that has selectable (`options`) or `optional` dependencies, Zoi will prompt you to make choices. To ensure that you can reinstall the same package with the exact same dependency choices later (for example, on another machine or in a CI/CD pipeline), Zoi automatically generates a sharable manifest file.

- **Location:** After a successful installation, a file named `<package_name>.manifest.yaml` is created inside the package's installation directory (`~/.zoi/pkgs/store/<package_name>/`).
- **Content:** This file records the package's name, version, and repository, along with the specific choices you made for its dependencies (`chosen_options` and `chosen_optionals`).

### Usage

You can copy this `.manifest.yaml` file into your project's repository. To perform a non-interactive, reproducible installation, simply run:

```sh
zoi install /path/to/your/package.manifest.yaml
```

Zoi will read the manifest, resolve the base package, and install it along with the exact set of dependencies specified in the file, skipping any interactive prompts.

## Troubleshooting

- Some managers require being present on PATH. Run `zoi info` to see which managers Zoi detects.
- On macOS, GUI apps use `brew-cask`; App Store apps require `mas` to be signed in.
- On Arch-based systems, you can choose between helpers (`yay`, `paru`, `pikaur`, `trizen`) or use `aur:` to build with `makepkg`.
