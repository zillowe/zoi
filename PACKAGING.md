# Packaging

This document provides guidance for packagers on how to build and package Zoi
for various distributions and package managers.

## Dependencies

Zoi has several dependencies that need to be installed for building from source
and for full functionality at runtime.

### Build-time Dependencies

These are required to compile Zoi from source.

- **Rust**: Current minimum version is `1.93.0` 2024 edition from the stable
channel (see [`rust-toolchain.toml`](./rust-toolchain.toml) for the channel and
[`Cargo.toml`](./Cargo.toml) for the Rust version and edition).
- **C Compiler**: A C compiler like `gcc` is required. Packages like
`build-essential` (Debian/Ubuntu) or `base-devel` (Arch Linux) usually provide this.
- **Clang/LLVM**: `clang` and `libclang-dev` (Debian/Ubuntu) or `clang-devel`
(Fedora) are required for `bindgen` (used by `openssl-sys` and other crates).
- **OpenSSL**: The development libraries for OpenSSL are required. This is
usually `libssl-dev` (Debian/Ubuntu) or `openssl-devel` (Fedora/CentOS).
- **pkg-config**: The `pkg-config` utility is needed to locate libraries.
- **liblzma**: The development libraries for lzma (`liblzma-dev`).
- **Git**: Required to embed the commit hash in the binary version information.

#### Runtime Dependencies

These are required for Zoi to run correctly after installation.

- **Essential:**
  - `git`: Required for interacting with git repositories
  - `gpg`: Required for verifying the authority of packages.
- **Optional:**
  - `bubblewrap`: The `bwrap` binary is required for building pure packages
  and running isolated apps.

## Build Process

Zoi can be built from source using several methods.

### Using Cargo

This is the standard way to build Rust projects. The build process can be
influenced by environment variables (see [Environment Variables](#environment-variables)).

```sh
# Build zoi in release mode
cargo build --bin zoi --release

# Build zoid (ZoiOS daemon) in release mode
cargo build --bin zoid --release
```

This will produce the `zoi` and `zoid` binaries in `target/release/`.

### Using the Justfile

The project provides a `Justfile` for convenience, which simplifies building
and installing.

```sh
# Configure build paths (creates config.just)
# You can also specify which binaries to build: --with-bin=zoi|zoi-mini|zoid|all (default: zoi)
./configure --prefix=/usr/local --with-bin=zoi

# Build release binaries
just build

# Install the binaries to the configured location
sudo just install
```

### Building .deb and .rpm Packages

Zoi supports generating `.deb` and `.rpm` packages for Linux distributions
using `cargo-deb` and `cargo-generate-rpm`.

#### Debian/Ubuntu (.deb)

To build a `.deb` package, ensure `cargo-deb` is installed:

```sh
cargo install cargo-deb
cargo deb -p zoi-rs
```

The resulting package will be located in `target/debian/`.

#### Fedora/RHEL (.rpm)

To build an `.rpm` package, ensure `cargo-generate-rpm` is installed:

```sh
cargo install cargo-generate-rpm
cargo generate-rpm -p zoi-rs
```

The resulting package will be located in `target/generate-rpm/`.

### Building the Docker Image Locally

A `Dockerfile` is provided to build Zoi in a containerized environment.
This is useful for creating reproducible builds or for custom image configurations.

```sh
# Build the docker image
docker build -t zoi .

# Build with custom telemetry keys (see Environment Variables)
docker build \
  --build-arg POSTHOG_API_KEY="your_key" \
  --build-arg POSTHOG_API_HOST="your_host" \
  --build-arg ZOI_DEFAULT_REGISTRY="https://my-registry.com/repo.git" \
  --build-arg ZOI_AUTHORITIES_KEY_1="trusted_fingerprint" \
  -t zoi .
```

### Using the Official Zoi CLI Docker Image

For CI/CD pipelines or environments where you need a pre-built Zoi CLI, an
official Docker image is available on the GitLab Container Registry.
This image contains the `zoi` binary and its runtime dependencies,
making it suitable for tasks like building Zoi packages.

The image is tagged with both the specific release version
(e.g. `zoi:Prod-Release-1.16.1`) and `zoi:latest`.

```sh
# Pull the latest Zoi CLI image
docker pull registry.gitlab.com/zillowe/zillwen/zusty/zoi/zoi:latest

# Example usage in a GitLab CI/CD job
my-job:
  image: registry.gitlab.com/zillowe/zillwen/zusty/zoi/zoi:latest
  script:
    - zoi package build my-package.pkg.lua --type source --platform linux-amd64
```

## Environment Variables

Zoi uses a few environment variables at build time.

- **`ZOI_COMMIT_HASH`**: Embeds the git commit hash into the binary.
This is used by the `zoi version` command. The build scripts in `scripts/`
set this automatically.
- **`POSTHOG_API_KEY`** & **`POSTHOG_API_HOST`**: These are used to configure
the optional, opt-in telemetry feature. They can be set in a `.env` file at
the root of the project or passed as build arguments to Docker.
The `.env.example` file shows the format.
- **`ZOI_DEFAULT_REGISTRY`**: Sets the default package registry URL.
This is used when no registry is configured by the user.
It can be set in a `.env` file or as a build argument to Docker.
- **`ZOI_AUTHORITIES_KEY_1`** to **`ZOI_AUTHORITIES_KEY_9`**: Sets the trusted
PGP fingerprints or key names for the default registry.
These define the "Root of Trust" for verifying Git commit signatures during `zoi sync`.
- **`ZOI_ABOUT_PACKAGER_AUTHOR`**, **`ZOI_ABOUT_PACKAGER_EMAIL`**,
**`ZOI_ABOUT_PACKAGER_HOMEPAGE`**: Allows a packager to embed their own contact
details into the binary. This information is displayed in the `zoi about`
command output, which is useful for users of a specific package to identify
the package maintainer.

## Built-in PGP Keyring

Zoi supports baking trusted PGP public keys directly into the binary.
Any `.asc` file placed in the `crates/core/src/builtin/pgp/` directory will
be embedded at build time.

On startup, Zoi automatically imports these embedded keys into the user's
local keyring (`<data-dir>/pgps/`, e.g. `~/.local/share/zoi/pgps/` on Linux;
see [Storage Locations](https://zillowe.qzz.io/docs/zds/zoi/storage.mdx)).
This is the recommended way to distribute "Root of Trust" keys for custom or internal registries.

## Built-in Registries

Zoi ships the official and supported package registries as YAML definitions
embedded into the binary, so registry resolution does not depend on a remote
central database at runtime. Applications can reference these by handle via
`zoi sync set <handle>` / `zoi sync add <handle>`.

1. Place a registry definition in `crates/core/src/builtin/registries/<handle>.yaml`.
   The file must declare a `handle`, `name`, `description`, `git` URL,
   `branch`, a `type` (`official` or `third-party`), and whether it is the
   single `set` registry.
2. Build Zoi as usual. The build system embeds every YAML file in that
   directory and resolves handles against them at runtime.

Exactly one built-in registry should be marked `set: true`; this becomes the
default registry that is used when none is configured by the user.

## Embedding Global Hooks

Similar to PGP keys, Zoi can embed global transaction hooks directly into
the binary. These hooks are YAML files that define system-wide maintenance
tasks triggered by file modifications.

1. Place your hook definition files (`.hook.yaml`) in the
`crates/core/src/builtin/hooks/` directory.
2. Build Zoi as usual.

The build system will automatically embed these hooks.
They are loaded on every transaction and can be overridden by users in
`<data-dir>/hooks/` (e.g. `~/.local/share/zoi/hooks/` on Linux;
see [Storage Locations](https://zillowe.qzz.io/docs/zds/zoi/storage.mdx)
if they use the same name.

## Completions and Man Pages

Zoi provides commands to generate shell completions and man pages.
These should be included in the package.

- **Shell Completions:**
  Completions can be generated for various shells using the `shell` command:

  ```sh
  ./target/release/zoi shell <shell> # generates completions and set them up for the user
  ```

  ```sh
  ./target/release/zoi generate-completions <shell> # generates completions and prints them
  ```

  Where `<shell>` can be `bash`, `fish`, `zsh`, etc.

- **Man Pages:**
  The man page sources are AsciiDoc files in the `man/` directory
  (`zoi.adoc`, `zoi-rs.adoc`, and `zoi-lua.adoc`).
  They are rendered to man pages with `asciidoctor` using the manpage
  backend. The rendered section is taken from the page title
  (e.g. `zoi.adoc` renders `zoi.1`).

  To render all man pages (equivalent to `just man`):

  ```sh
  asciidoctor -b manpage -D dist/man man/*.adoc
  ```

  To render a single page:

  ```sh
  mkdir -p dist/man
  asciidoctor -b manpage -D dist/man man/zoi.adoc
  asciidoctor -b manpage -D dist/man man/zoi-rs.adoc
  asciidoctor -b manpage -D dist/man man/zoi-lua.adoc
  ```

  This produces `dist/man/zoi.1`, `dist/man/zoi-rs.3`, and
  `dist/man/zoi-lua.5`. Install each page into the matching man directory
  (e.g. `zoi.1` goes into `/usr/share/man/man1`, `zoi-rs.3` into
  `/usr/share/man/man3`, and `zoi-lua.5` into `/usr/share/man/man5`).

  Pre-built binary archives do not include man pages. Source-based packages
  should render them from the `man/` directory at build time with
  `asciidoctor`; require it as a build dependency (`rubygem-asciidoctor` on
  Fedora/RHEL, `asciidoctor` on Arch Linux).

## Existing Packaging Files

We maintain packaging files for several package managers in the `packages/`
directory. These can be used as a reference.

### Arch Linux (AUR)

- [`zoi`](./packages/aur/zoi/PKGBUILD): For building from source.
- [`zoi-bin`](./packages/aur/zoi-bin/PKGBUILD): For packaging pre-compiled binaries.

### Homebrew

- [`zoi.rb`](./packages/brew/zoi.rb): Homebrew formula.

### Scoop

- [`zoi.json`](./packages/scoop/zoi.json): Scoop manifest for Windows.

### RHEL

- [`zoi.spec`](./packages/rpm/zoi.spec): RPM spec file for RHEL distros.

## Packaging Status

[![Packaging status](https://repology.org/badge/vertical-allrepos/zoi.svg)](https://repology.org/project/zoi/versions)
