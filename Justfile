#!/usr/bin/env -S just --working-directory . --justfile

set shell := ["bash", "-c"]

set dotenv-load
set dotenv-filename := "config.just"

PREFIX := env("PREFIX", "/usr/local")
PROFILE := env("PROFILE", "release")
OS_NAME := env("OS_NAME", shell("uname -s | tr '[:upper:]' '[:lower:]'"))
ARCH_NAME := env("ARCH_NAME", shell("uname -m"))
WITH_BIN := env("WITH_BIN", "both")

# Cargo packages for WITH_BIN. Packages are named with -p because the
# workspace's default-members is only zoi-rs, so `--bin zoi-mini` or
# `--bin zoid` would not be found.
CARGO_PKGS := if WITH_BIN == "zoi" { "-p zoi-rs" } else if WITH_BIN == "zoi-mini" { "-p zoi-mini" } else if WITH_BIN == "zoid" { "-p zoi-daemon" } else if WITH_BIN == "both" { "-p zoi-rs -p zoi-mini" } else { "-p zoi-rs -p zoi-mini -p zoi-daemon" }

BINDIR := env("BINDIR", PREFIX + "/bin")
MANDIR := env("MANDIR", PREFIX + "/share/man")
DEBUGDIR := env("DEBUGDIR", "dist")
DEV_BINDIR := DEBUGDIR + "/bin"
DEV_MANDIR := DEBUGDIR + "/man"
NAME := "zoi"

MINI_NAME := "zoi-mini"
DAEMON_NAME := "zoid"
# The checkout's commit from git; outside a git checkout (a source tarball),
# `ZOI_COMMIT_HASH`, else the `COMMIT` file the release tarball ships. Git
# comes first so a stale exported variable cannot mislabel a build.
COMMIT_HASH := shell("git rev-parse --short=10 HEAD 2>/dev/null || printf '%s' \"${ZOI_COMMIT_HASH:-}\" | grep . || cat COMMIT 2>/dev/null || echo unknown")
IS_WINDOWS := if OS_NAME == "windows" { "1" } else { "0" }
EXE_EXT := if IS_WINDOWS == "1" { ".exe" } else { "" }
SRC_BIN := "target/" + PROFILE + "/" + NAME + EXE_EXT
MINI_SRC_BIN := "target/" + PROFILE + "/" + MINI_NAME + EXE_EXT
DAEMON_SRC_BIN := "target/" + PROFILE + "/" + DAEMON_NAME + EXE_EXT
DEBUG_SRC_BIN := "target/debug/" + NAME + EXE_EXT
MINI_DEBUG_SRC_BIN := "target/debug/" + MINI_NAME + EXE_EXT
DAEMON_DEBUG_SRC_BIN := "target/debug/" + DAEMON_NAME + EXE_EXT

export RUSTFLAGS := "-Dwarnings"
export RUST_BACKTRACE := "1"

[private]
_is_configured := if path_exists("config.just") == "true" { "true" } else { "false" }

# Alias to `just help`
default: help

# Build Zoi in release mode
build:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v clang >/dev/null 2>&1; then echo "Error: 'clang' is not installed. It is required for bindgen."; exit 1; fi
    @echo "Building Zoi targets: {{ WITH_BIN }} in {{ PROFILE }} mode (commit: {{ COMMIT_HASH }})..."; \
    set -a; source .env 2>/dev/null; export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; \
    cargo build {{ CARGO_PKGS }} --profile {{ PROFILE }}
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

# Build Zoi in debug mode
dev:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v clang >/dev/null 2>&1; then echo "Error: 'clang' is not installed. It is required for bindgen."; exit 1; fi
    @echo "Building Zoi targets: {{ WITH_BIN }} in debug mode (commit: {{ COMMIT_HASH }})..."; \
    set -a; source .env 2>/dev/null; export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; \
    cargo build {{ CARGO_PKGS }}
    @mkdir -p "{{ DEV_BINDIR }}"
    @if [ "{{ WITH_BIN }}" = "zoi" ] || [ "{{ WITH_BIN }}" = "all" ] || [ "{{ WITH_BIN }}" = "both" ]; then \
        cp -f "{{ DEBUG_SRC_BIN }}" "{{ DEV_BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
        echo "Zoi (debug) copied to {{ DEV_BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" = "zoi-mini" ] || [ "{{ WITH_BIN }}" = "all" ] || [ "{{ WITH_BIN }}" = "both" ]; then \
        cp -f "{{ MINI_DEBUG_SRC_BIN }}" "{{ DEV_BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
        echo "Zoi Mini (debug) copied to {{ DEV_BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" = "zoid" ] || [ "{{ WITH_BIN }}" = "all" ]; then \
        cp -f "{{ DAEMON_DEBUG_SRC_BIN }}" "{{ DEV_BINDIR }}/{{ DAEMON_NAME }}{{ EXE_EXT }}"; \
        echo "Zoid (debug) copied to {{ DEV_BINDIR }}/{{ DAEMON_NAME }}{{ EXE_EXT }}"; \
    fi
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

# Install Zoi binary to `PREFIX` or default user's bin location
install:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Installing requested binaries to {{ BINDIR }}..."
    @mkdir -p "{{ BINDIR }}"
    @if [ "{{ WITH_BIN }}" = "zoi" ] || [ "{{ WITH_BIN }}" = "all" ] || [ "{{ WITH_BIN }}" = "both" ]; then \
        if [ "{{ IS_WINDOWS }}" = "1" ]; then \
            cp -f "{{ SRC_BIN }}" "{{ BINDIR }}/{{ NAME }}.exe"; \
        else \
            install -m 755 "{{ SRC_BIN }}" "{{ BINDIR }}/{{ NAME }}"; \
        fi; \
        echo "Zoi installed successfully to {{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" = "zoi-mini" ] || [ "{{ WITH_BIN }}" = "all" ] || [ "{{ WITH_BIN }}" = "both" ]; then \
        if [ "{{ IS_WINDOWS }}" = "1" ]; then \
            cp -f "{{ MINI_SRC_BIN }}" "{{ BINDIR }}/{{ MINI_NAME }}.exe"; \
        else \
            install -m 755 "{{ MINI_SRC_BIN }}" "{{ BINDIR }}/{{ MINI_NAME }}"; \
        fi; \
        echo "Zoi Mini installed successfully to {{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" = "zoid" ] || [ "{{ WITH_BIN }}" = "all" ]; then \
        if [ "{{ IS_WINDOWS }}" = "1" ]; then \
            echo "Error: zoid is not supported on Windows."; \
        else \
            install -m 755 "{{ DAEMON_SRC_BIN }}" "{{ BINDIR }}/{{ DAEMON_NAME }}"; \
            echo "Zoid installed successfully to {{ BINDIR }}/{{ DAEMON_NAME }}{{ EXE_EXT }}"; \
            if [ -d "/usr/lib/systemd/system" ]; then \
                sudo install -m 644 "crates/daemon/zoid.service" "/usr/lib/systemd/system/zoid.service"; \
                sudo install -Dm 644 "crates/daemon/org.freedesktop.PackageKit.conf" "/usr/share/dbus-1/system.d/org.freedesktop.PackageKit.conf"; \
                echo "Zoid systemd service and D-Bus policy installed."; \
                echo "Run 'sudo systemctl enable --now zoid' to start it at boot."; \
            fi; \
        fi; \
    fi
    @echo "Make sure '{{ BINDIR }}' is in your PATH."

# Install the rendered man pages to `MANDIR`
install-man:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v asciidoctor >/dev/null 2>&1; then echo "Error: 'asciidoctor' is not installed. It is required to render man pages."; exit 1; fi
    @echo "Rendering man pages..."
    @./scripts/man.sh "{{ DEV_MANDIR }}"
    @echo "Installing man pages to {{ MANDIR }}..."
    @sudo install -d -m 755 "{{ MANDIR }}/man1" "{{ MANDIR }}/man3" "{{ MANDIR }}/man5" "{{ MANDIR }}/man8"
    @sudo install -m 644 "{{ DEV_MANDIR }}/zoi.1" "{{ MANDIR }}/man1/zoi.1"
    @sudo install -m 644 "{{ DEV_MANDIR }}/zoi-rs.3" "{{ MANDIR }}/man3/zoi-rs.3"
    @sudo install -m 644 "{{ DEV_MANDIR }}/zoi-lua.5" "{{ MANDIR }}/man5/zoi-lua.5"
    @sudo install -m 644 "{{ DEV_MANDIR }}/zoid.8" "{{ MANDIR }}/man8/zoid.8"
    @echo "Man pages installed."

# Uninstall Zoi binary
uninstall:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Uninstalling binaries from {{ BINDIR }}..."
    @rm -f "{{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ DAEMON_NAME }}{{ EXE_EXT }}"
    @if [ -f "/usr/lib/systemd/system/zoid.service" ]; then \
        sudo systemctl disable --now zoid 2>/dev/null || true; \
        sudo rm -f "/usr/lib/systemd/system/zoid.service"; \
        sudo rm -f "/usr/share/dbus-1/system.d/org.freedesktop.PackageKit.conf"; \
        sudo rm -rf /run/zoid; \
        echo "Zoid systemd service removed."; \
    fi
    @echo "Binaries uninstalled."

# Uninstall the rendered man pages from `MANDIR`
uninstall-man:
    @echo "Removing man pages from {{ MANDIR }}..."
    @sudo rm -f "{{ MANDIR }}/man1/zoi.1"
    @sudo rm -f "{{ MANDIR }}/man3/zoi-rs.3"
    @sudo rm -f "{{ MANDIR }}/man5/zoi-lua.5"
    @sudo rm -f "{{ MANDIR }}/man8/zoid.8"
    @echo "Man pages removed."

# Generate man pages from the asciidoc sources in man/
man:
    @./scripts/man.sh "{{ DEV_MANDIR }}"

# Generate shell completions from the release build into DEBUGDIR/completions
completions:
    @if [ ! -x "{{ SRC_BIN }}" ]; then echo "Error: '{{ SRC_BIN }}' not found. Run 'just build' first."; exit 1; fi
    @mkdir -p "{{ DEBUGDIR }}/completions"
    @"{{ SRC_BIN }}" generate-completions bash > "{{ DEBUGDIR }}/completions/{{ NAME }}.bash"
    @"{{ SRC_BIN }}" generate-completions zsh > "{{ DEBUGDIR }}/completions/_{{ NAME }}"
    @"{{ SRC_BIN }}" generate-completions fish > "{{ DEBUGDIR }}/completions/{{ NAME }}.fish"
    @"{{ SRC_BIN }}" generate-completions elvish > "{{ DEBUGDIR }}/completions/{{ NAME }}.elv"
    @"{{ SRC_BIN }}" generate-completions powershell > "{{ DEBUGDIR }}/completions/_{{ NAME }}.ps1"
    @echo "Shell completions written to {{ DEBUGDIR }}/completions."

# Create the source tarball zoi-<version>.tar.gz (from the last commit) in DEBUGDIR
tarball:
    @./scripts/source-tarball.sh "{{ DEBUGDIR }}"

# Clean project artifacts
clean:
    @echo "Cleaning project artifacts..."
    @cargo clean
    @rm -rf dist/
    @echo "Cleaned."

# Configure the Justfile
configure *args:
    ./configure {{ args }}

# Check for unused dependencies
deps:
    @cargo machete --with-metadata

# Format Rust code
fmt:
    @if rustfmt --version 2>/dev/null | grep -q 'nightly'; then \
        cargo fmt --all; \
    elif cargo +nightly --version >/dev/null 2>&1; then \
        cargo +nightly fmt --all; \
    else \
        echo "Error: Nightly rustfmt is required. Please enter the Nix dev shell or install nightly via rustup."; \
        exit 1; \
    fi

# Check for formating issues in Rust code
fmt-check:
    @if rustfmt --version 2>/dev/null | grep -q 'nightly'; then \
        cargo fmt --all --check; \
    elif cargo +nightly --version >/dev/null 2>&1; then \
        cargo +nightly fmt --all --check; \
    else \
        echo "Error: Nightly rustfmt is required. Please enter the Nix dev shell or install nightly via rustup."; \
        exit 1; \
    fi

# Check Rust code for error
check:
    @cargo check --workspace --all-targets --tests

# Run lints on Rust code
lint:
    @cargo clippy --workspace --all-targets --all-features

# Run lints on Rust code and apply fixes where possible
lint-fix:
    @cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged

# Run Zoi test suite
test:
    @cargo test --workspace --all-targets --all-features

# Enter the Nix development shell with the current shell (or run a command)
nix *args:
    @if [ -z "{{ args }}" ]; then \
        nix develop -c "$SHELL"; \
    else \
        nix develop --command {{ args }}; \
    fi

# Bump Zoi version
bump *args:
    @./scripts/bump.sh {{ args }}

# Print this help message
help:
    @just --list
