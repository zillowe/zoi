# Packaging

This document provides guidance for packagers on how to build and package Zoi for various distributions and package managers.

## Dependencies

Zoi has several dependencies that need to be installed for building from source and for full functionality at runtime.

#### Build-time Dependencies

These are required to compile Zoi from source.

- **Rust**: Current minimum version is `1.88.0` 2024 edition from the stable channel (see [`rust-toolchan.toml`](./rust-toolchain.toml) for the channel and [`Cargo.toml`](./Cargo.toml) for the Rust version and edition).
- **C Compiler**: A C compiler like `gcc` is required. Packages like `build-essential` (Debian/Ubuntu) or `base-devel` (Arch Linux) usually provide this.
- **OpenSSL**: The development libraries for OpenSSL are required. This is usually `libssl-dev` (Debian/Ubuntu) or `openssl-devel` (Fedora/CentOS).
- **pkg-config**: The `pkg-config` utility is needed to locate libraries.
- **liblzma**: The development libraries for lzma (`liblzma-dev`).
- **Git**: Required to embed the commit hash in the binary version information.

#### Runtime Dependencies

These are required for Zoi to run correctly after installation.

- **Essential:**
  - `git`: Required for interacting with git repositories (e.g. cloning packages, syncing the database).
- **Optional:**
  - `less`: Used for viewing files within Zoi, for example when showing a package's manual.

## Build Process

Zoi can be built from source using several methods.

### Using Cargo

This is the standard way to build Rust projects. The build process can be influenced by environment variables (see [Environment Variables](#environment-variables)).

```sh
# Build the main zoi binary in release mode
cargo build --bin zoi --release
```

This will produce the `zoi` binary in `target/release/`. To build all binaries (`zoi`, `zoi-completions`, `zoi-mangen`), run:

```sh
cargo build --release
```

### Using the Makefile

The project provides a `Makefile` for convenience, which simplifies building and installing.

```sh
# Configure build paths (creates config.mk)
./configure

# Build the zoi binary in release mode
make build

# Install the binary to the configured location
sudo make install
```

### Using Build Scripts

The `scripts/` directory contains scripts for creating release builds for different platforms. These are used in our CI/CD pipeline.

- `scripts/build-linux.sh`: Builds for Linux (and64, arm64) and cross-compiles for Windows (amd64).
- `scripts/build-macos.sh`: Builds for macOS (amd64, arm64).
- `scripts/build-release.sh` & `build-release.ps1`: Helper scripts for creating a single release build on the current platform.

These scripts embed the current git commit hash into the binary via the `ZOI_COMMIT_HASH` environment variable.

### Using Docker

A `Dockerfile` is provided to build Zoi in a containerized environment. This is useful for creating reproducible builds.

```sh
# Build the docker image
docker build -t zoi .

# Build with custom telemetry keys (see Environment Variables)
docker build \
  --build-arg POSTHOG_API_KEY="your_key" \
  --build-arg POSTHOG_API_HOST="your_host" \
  --build-arg ZOI_DEFAULT_REGISTRY="https://my-registry.com/repo.git" \
  -t zoi .
```

## Environment Variables

Zoi uses a few environment variables at build time.

- **`ZOI_COMMIT_HASH`**: Embeds the git commit hash into the binary. This is used by the `zoi version` command. The build scripts in `scripts/` set this automatically.
- **`POSTHOG_API_KEY`** & **`POSTHOG_API_HOST`**: These are used to configure the optional, opt-in telemetry feature. They can be set in a `.env` file at the root of the project or passed as build arguments to Docker. The `.env.example` file shows the format.
- **`ZOI_DEFAULT_REGISTRY`**: Sets the default package registry URL. This is used when no registry is configured by the user. It can be set in a `.env` file or as a build argument to Docker.

## Binaries

The build process generates three binaries:

- `zoi`: The main application binary.
- `zoi-completions`: A helper binary to generate shell completion scripts.
- `zoi-mangen`: A helper binary to generate the man page.

For most packaging purposes, you will only need to package the `zoi` binary, as it can also generate completions and man pages itself.

## Completions and Man Pages

Zoi provides commands to generate shell completions and man pages. These should be included in the package.

- **Shell Completions:**
  Completions can be generated for various shells using the `shell` command:

  ```sh
  ./target/release/zoi shell <shell> # generates completions and set them up for the user
  ```

  ```sh
  ./target/release/zoi generate-completions <shell> # generates completions and prints them
  ```

  Where `<shell>` can be `bash`, `fish`, `zsh`, etc.

- **Man Page:**
  The man page can be generated using the `generate-manual` command (which is an alias for `zoi-mangen` but prints it instead):
  ```sh
  ./target/release/zoi generate-manual > zoi.1
  ```

Alternatively, the `zoi-completions` and `zoi-mangen` binaries can be used directly:

```sh
OUT_DIR=dist/completions/ ./target/release/zoi-completions
OUT_DIR=dis/man/ ./target/release/zoi-mangen
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
