# Packaging

This document provides guidance for packagers on how to build and package Zoi for various distributions and package managers.

## Dependencies

Zoi has several dependencies that need to be installed for building from source and for full functionality at runtime.

#### Build-time Dependencies

These are required to compile Zoi from source.

- **Rust Toolchain**: See `rust-toolchain.toml` for the specific version.
- **C Compiler**: A C compiler like `gcc` is required. Packages like `build-essential` (Debian/Ubuntu) or `base-devel` (Arch Linux) usually provide this.
- **OpenSSL**: The development libraries for OpenSSL are required. This is usually `libssl-dev` (Debian/Ubuntu) or `openssl-devel` (Fedora/CentOS).
- **pkg-config**: The `pkg-config` utility is needed to locate libraries.
- **liblzma**: The development libraries for lzma (`liblzma-dev`).

#### Runtime Dependencies

These are required for Zoi to run correctly after installation.

- **Essential:**
  - `git`: Required for interacting with git repositories (e.g. cloning packages, syncing the database).
- **Optional:**
  - `less`: Used for viewing files within Zoi, for example when showing a package's manual.

## Build Process

Zoi can be built from source using Cargo.

### Dependencies

- Rust toolchain (see `rust-toolchain.toml`)
- `make`
- `gcc`

### Building from source

The project can be built in two ways:

1.  **Using Cargo:**
    This is the standard way to build Rust projects.

    ```sh
    cargo build --release
    ```

    This will produce the main `zoi` binary in `target/release/zoi`.
    The build can be configured for release builds in `.cargo/config.toml`.

2.  **Using Makefile:**
    The project also provides a `Makefile` for convenience.
    ```sh
    ./configure
    make build
    ```
    This will also build the project in release mode. The `Makefile` can also be used to install Zoi locally.

### Binaries

The build process generates several binaries defined in `Cargo.toml`:

- `zoi`: The main application binary.
- `zoi-completions`: A helper binary to generate shell completion scripts.
- `zoi-mangen`: A helper binary to generate the man page.

### Completions and Man Pages

Zoi provides commands to generate shell completions and man pages. These should be included in the package.

- **Shell Completions:**
  Completions can be generated for various shells using the `generate-completions` command:

  ```sh
  ./target/release/zoi generate-completions <shell>
  ```

  Where `<shell>` can be `bash`, `fish`, `zsh`, etc.

- **Man Page:**
  The man page can be generated using the `generate-manual` command:
  ```sh
  ./target/release/zoi generate-manual > zoi.1
  ```

Alternatively, the `zoi-completions` and `zoi-mangen` binaries can be used directly:

```sh
OUT_DIR=completions/ ./target/release/zoi-completions
OUT_DIR=man/ ./target/release/zoi-mangen
```

## Existing Packaging Files

We maintain packaging files for several package managers in the `packages/` directory. These can be used as a reference.

### Arch Linux (AUR)

- [`zoi`](./packages/aur/zoi/PKGBUILD): For building from source.
- [`zoi-bin`](./packages/aur/zoi-bin/PKGBUILD): For packaging pre-compiled binaries.

### Homebrew

- [`zoi.rb`](./packages/brew/zoi.rb): Homebrew formula.

### Scoop

- [`zoi.json`](./packages/scoop/zoi.json): Scoop manifest for Windows.
