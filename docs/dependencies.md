---
title: Dependencies & Supported Package Managers
description: How Zoi installs dependencies from external package managers and the full set of supported managers.
---

Zoi can install dependencies via many ecosystem and OS package managers. This page documents:

- How to declare dependencies in `pkg.yaml`
- All supported managers, platforms, and the commands Zoi runs under the hood
- Notes and caveats for specific managers

## Declaring dependencies in `pkg.yaml`

Dependencies are specified in the `dependencies` section. Both `runtime` and `build` support:

- required: always installed
- optional: user-confirmed
- options: selectable groups (providers)

```yaml
dependencies:
  runtime:
    required:
      - native:openssl
      - npm:typescript
      - zoi:some-zoi-package
    optional:
      - pipx:black:Python formatter CLI
  build:
    required:
      - native:make
    options:
      - name: "GUI Toolkit"
        desc: "Choose GUI provider"
        all: false
        depends:
          - native:qt6:for KDE desktops
          - native:gtk4:for GNOME desktops
```

Format per entry: `manager:package` with optional version and an optional inline description after the last colon.

- Version can be specified either with `@<semver>` or with a comparator string like `=1.2.3`, `>=2.0.0`, `^1.2`, `~1.2.3`.
- Do not prefix versions with `v`. Use `@1.2.3`, not `@v1.2.3`.
- Examples:
  - `npm:typescript@5.3.2`
  - `cargo:bat@0.24.0`
  - `apt:curl=7.68.0-1ubuntu2.18`
  - `zoi:some-zoi-package@1.2.3`
  - `zoi:some-zoi-package@stable` (uses that package's `versions` map)
- Inline description goes after the last colon if present and must not contain version characters. For example: `pipx:black:Python formatter CLI`.
- For system-native packages, Zoi picks your OS package manager automatically when using `native:<pkg>`.

Notes on versions and managers:

- Zoi parses versions using SemVer requirements. If a manager cannot enforce versions directly, Zoi attempts best-effort checks or warns that pinning may not be honored.
- Some managers support pinning (e.g. `apt` via `pkg=ver`, `dnf` via `pkg-ver`, `choco` via `--version`, `cargo` via `--version`, `npm/yarn/pnpm/bun` via `pkg@ver`, `brew` when a formula tap provides `pkg@ver`).
- Many OS managers do not support explicit pinning in a reliable way (e.g. `pacman`, `yay/paru`, `apk`, `xbps`, `eopkg`, `guix`, `portage`, `snap`, `flatpak`, `macports`, `conda`). Zoi will install the latest available and may print a warning.
- Go modules: Zoi currently installs with `go install <module>@latest`. Declaring a version like `go:module@...` is not supported through Zoi's SemVer parser and may fail.

Templating:

- You can reference the parent package version using `{version}` inside a dependency string. It will be replaced before parsing. Example: `zoi:my-plugin@{version}`.

Mapped versions (versions map):

- If your package defines a `versions:` map (channel → concrete version), Zoi resolves the concrete version first and then substitutes it into dependencies via `{version}`.
- This lets you keep dependency versions in lockstep with the resolved package version. For example, if `stable` maps to `1.4.3`, then `{version}` becomes `1.4.3` in dependencies.
- Channel tokens inside dependencies are supported only for `zoi:` dependencies. You can write `zoi:<pkg>@<channel>` (e.g. `zoi:core-tool@stable`) and Zoi will resolve that package's `versions` map.
- For non-`zoi` managers (e.g. `npm`, `apt`, `cargo`), channel names like `@stable` are not supported; use `{version}` templating instead.

Example:

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

In this example, Zoi resolves `my-app` to its concrete version from `versions:` (defaults to `stable` if present) and substitutes that into the dependency strings before installing.

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

## Troubleshooting

- Some managers require being present on PATH. Run `zoi info` to see which managers Zoi detects.
- On macOS, GUI apps use `brew-cask`; App Store apps require `mas` to be signed in.
- On Arch-based systems, you can choose between helpers (`yay`, `paru`, `pikaur`, `trizen`) or use `aur:` to build with `makepkg`.
