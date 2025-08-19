---
title: Dependencies & Supported Package Managers
description: How Zoi installs dependencies from external package managers and the full set of supported managers.
---

Zoi can install dependencies via many ecosystem and OS package managers. This page documents:

- How to declare dependencies in `pkg.yaml`
- All supported managers, platforms, and the commands Zoi runs under the hood
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

Legend:

- Platform: `linux/macos/windows/freebsd/openbsd`; or distro family
- Install/Uninstall: the exact commands Zoi runs

| Manager          | Platform/family        | Install command (approx)                    | Uninstall command (approx)           |
| ---------------- | ---------------------- | ------------------------------------------- | ------------------------------------ |
| `zoi`            | all                    | `zoi install`                               | `zoi uninstall`                      |
| `native`         | auto (OS/distro)       | picks the default package manager           | picks corresponding remove command   |
| `apt`, `apt-get` | Debian/Ubuntu          | `sudo apt install -y` `<pkg>`               | `sudo apt remove -y <pkg>`           |
| `pacman`         | Arch                   | `sudo pacman -S --needed --noconfirm <pkg>` | `sudo pacman -Rns --noconfirm <pkg>` |
| `yay`            | Arch (AUR helper)      | `yay -S --needed --noconfirm <pkg>`         | `yay -Rns --noconfirm <pkg>`         |
| `paru`           | Arch (AUR helper)      | `paru -S --needed --noconfirm <pkg>`        | `paru -Rns --noconfirm <pkg>`        |
| `pikaur`         | Arch (AUR helper)      | `pikaur -S --needed --noconfirm <pkg>`      | `pikaur -Rns --noconfirm <pkg>`      |
| `trizen`         | Arch (AUR helper)      | `trizen -S --needed --noconfirm <pkg>`      | `trizen -Rns --noconfirm <pkg>`      |
| `aur`            | Arch (AUR via makepkg) | `git clone + makepkg -si`                   | `pacman -Rns --noconfirm <pkg>`      |
| `dnf`, `yum`     | Fedora/RHEL            | `sudo dnf install -y <pkg>`                 | `sudo dnf remove -y <pkg>`           |
| `zypper`         | openSUSE               | `sudo zypper install -y <pkg>`              | `sudo zypper remove -y <pkg>`        |
| `apk`            | Alpine                 | `sudo apk add <pkg>`                        | `sudo apk del <pkg>`                 |
| `portage`        | Gentoo                 | `sudo emerge <pkg>`                         | `sudo emerge --unmerge <pkg>`        |
| `xbps-install`   | Void Linux             | `sudo xbps-install -S <pkg>`                | `sudo xbps-remove -R <pkg>`          |
| `eopkg`          | Solus                  | `sudo eopkg it -y <pkg>`                    | `sudo eopkg rm -y <pkg>`             |
| `guix`           | GNU Guix               | `guix install <pkg>`                        | `guix remove <pkg>`                  |
| `brew`           | macOS                  | `brew install <pkg>`                        | `brew uninstall <pkg>`               |
| `brew-cask`      | macOS (GUI apps)       | `brew install --cask <pkg>`                 | `brew uninstall --cask <pkg>`        |
| `mas`            | macOS App Store        | `mas install <id-or-name>`                  | `mas remove <id-or-name>`            |
| `macports`       | macOS                  | `sudo port install <pkg>`                   | `sudo port uninstall <pkg>`          |
| `scoop`          | Windows                | `scoop install <pkg>`                       | `scoop uninstall <pkg>`              |
| `choco`          | Windows                | `choco install -y <pkg>`                    | `choco uninstall -y <pkg>`           |
| `winget`         | Windows                | `winget install <pkg> --silent`             | `winget uninstall <pkg> --silent`    |
| `snap`           | Linux (Snap)           | `sudo snap install <pkg>`                   | `sudo snap remove <pkg>`             |
| `flatpak`        | Linux (Flathub)        | `sudo flatpak install flathub <pkg> -y`     | `flatpak uninstall -y <pkg>`         |
| `pkg`            | FreeBSD                | `sudo pkg install -y <pkg>`                 | `sudo pkg delete -y <pkg>`           |
| `pkg_add`        | OpenBSD                | `sudo pkg_add -I <pkg>`                     | `sudo pkg_delete <pkg>`              |
| `cargo`          | Rust                   | `cargo install <crate>`                     | `cargo uninstall <crate>`            |
| `cargo-binstall` | Rust (binary)          | `cargo binstall <crate>`                    | `cargo uninstall <crate>`            |
| `go`             | Go                     | `go install <module>@latest`                | (no uninstall; manual)               |
| `npm`            | Node.js                | `npm install -g <pkg>`                      | `npm uninstall -g <pkg>`             |
| `yarn`           | Node.js                | `yarn global add <pkg>`                     | `yarn global remove <pkg>`           |
| `pnpm`           | Node.js                | `pnpm add -g <pkg>`                         | `pnpm remove -g <pkg>`               |
| `bun`            | Bun                    | `bun install -g <pkg>`                      | `bun remove -g <pkg>`                |
| `volta`          | JavaScript             | `volta install <pkg>`                       | (no uninstall)                       |
| `deno`           | Deno                   | `deno install -g <pkg>`                     | `deno uninstall <pkg>`               |
| `jsr`            | JavaScript Registry    | `npx jsr add <pkg>`                         | (no uninstall)                       |
| `pip`            | Python                 | `pip install <pkg>`                         | `pip uninstall -y <pkg>`             |
| `pipx`           | Python CLI tools       | `pipx install <pkg>`                        | `pipx uninstall <pkg>`               |
| `uv`             | Python CLI tools       | `uv tool install <pkg>`                     | `uv tool uninstall <pkg>`            |
| `conda`          | Conda                  | `conda install -y <pkg>`                    | `conda uninstall -y <pkg>`           |
| `gem`            | Ruby                   | `gem install <pkg>`                         | `gem uninstall <pkg>`                |
| `composer`       | PHP                    | `composer global require <pkg>`             | `composer global remove <pkg>`       |
| `dotnet`         | .NET                   | `dotnet tool install -g <pkg>`              | `dotnet tool uninstall -g <pkg>`     |
| `nix`            | Nix                    | `nix-env -iA nixpkgs.<pkg>`                 | `nix-env -e <pkg>`                   |
| `dart-pub`       | Dart                   | `dart pub global activate <pkg>`            | `dart pub global deactivate <pkg>`   |

Notes:

- AUR: `aur:<pkg>` builds from source using `makepkg`; uninstall is done with `pacman`.
- `native:<pkg>` selects the appropriate system manager based on OS/distro; if none can be detected, Zoi errors.
- Some managers (e.g. `go`, `jsr`, `volta`) do not provide reliable uninstall; Zoi prints a notice and skips.
- The `script` manager takes a URL as the package name (e.g. `script:https://example.com/install`). It appends `.sh` for Linux/macOS and `.ps1` for Windows, then downloads and executes the script. There is no automatic uninstallation.

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
