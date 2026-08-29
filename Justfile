#!/usr/bin/env -S just --working-directory . --justfile

set shell := ["bash", "-c"]

set dotenv-load
set dotenv-filename := "config.just"

PREFIX := env("PREFIX", "/usr/local")
PROFILE := env("PROFILE", "release")
OS_NAME := env("OS_NAME", shell("uname -s | tr '[:upper:]' '[:lower:]'"))
ARCH_NAME := env("ARCH_NAME", shell("uname -m"))
WITH_BIN := env("WITH_BIN", "both")

BINDIR := env("BINDIR", PREFIX + "/bin")
DEBUGDIR := env("DEBUGDIR", "dist")
DEV_BINDIR := DEBUGDIR + "/bin"
DEV_MANDIR := DEBUGDIR + "/man"
NAME := "zoi"

MINI_NAME := "zoi-mini"
DAEMON_NAME := "zoid"
COMMIT_HASH := shell("git rev-parse --short=10 HEAD")
IS_WINDOWS := if OS_NAME == "windows" { "1" } else { "0" }
EXE_EXT := if IS_WINDOWS == "1" { ".exe" } else { "" }
SRC_BIN := "target/" + PROFILE + "/" + NAME + EXE_EXT
MINI_SRC_BIN := "target/" + PROFILE + "/" + MINI_NAME + EXE_EXT
DAEMON_SRC_BIN := "target/" + PROFILE + "/" + DAEMON_NAME + EXE_EXT
DEBUG_SRC_BIN := "target/debug/" + NAME + EXE_EXT
MINI_DEBUG_SRC_BIN := "target/debug/" + MINI_NAME + EXE_EXT
DAEMON_DEBUG_SRC_BIN := "target/debug/" + DAEMON_NAME + EXE_EXT

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
    if [ "{{ WITH_BIN }}" = "zoi" ]; then \
        cargo build --bin zoi --profile {{ PROFILE }}; \
    elif [ "{{ WITH_BIN }}" = "zoi-mini" ]; then \
        cargo build --bin zoi-mini --profile {{ PROFILE }}; \
    elif [ "{{ WITH_BIN }}" = "zoid" ]; then \
        cargo build --bin zoid --profile {{ PROFILE }}; \
    elif [ "{{ WITH_BIN }}" = "both" ]; then \
        cargo build --bin zoi --bin zoi-mini --profile {{ PROFILE }}; \
    else \
        cargo build --bin zoi --bin zoi-mini --bin zoid --profile {{ PROFILE }}; \
    fi
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

# Build Zoi in debug mode
dev:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v clang >/dev/null 2>&1; then echo "Error: 'clang' is not installed. It is required for bindgen."; exit 1; fi
    @echo "Building Zoi targets: {{ WITH_BIN }} in debug mode (commit: {{ COMMIT_HASH }})..."; \
    set -a; source .env 2>/dev/null; export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; \
    if [ "{{ WITH_BIN }}" = "zoi" ]; then \
        cargo build --bin zoi; \
    elif [ "{{ WITH_BIN }}" = "zoi-mini" ]; then \
        cargo build --bin zoi-mini; \
    elif [ "{{ WITH_BIN }}" = "zoid" ]; then \
        cargo build --bin zoid; \
    elif [ "{{ WITH_BIN }}" = "both" ]; then \
        cargo build --bin zoi --bin zoi-mini; \
    else \
        cargo build --bin zoi --bin zoi-mini --bin zoid; \
    fi
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
                echo "Zoid systemd service installed."; \
            fi; \
        fi; \
    fi
    @echo "Make sure '{{ BINDIR }}' is in your PATH."

# Uninstall Zoi binary
uninstall:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Uninstalling binaries from {{ BINDIR }}..."
    @rm -f "{{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ DAEMON_NAME }}{{ EXE_EXT }}"
    @if [ -f "/usr/lib/systemd/system/zoid.service" ]; then \
        sudo rm -f "/usr/lib/systemd/system/zoid.service"; \
        echo "Zoid systemd service removed."; \
    fi
    @echo "Binaries uninstalled."

# Generate man pages from the asciidoc sources in man/
man:
    @if ! command -v asciidoctor >/dev/null 2>&1; then echo "Error: 'asciidoctor' is not installed. It is required to render man pages."; exit 1; fi
    @mkdir -p "{{ DEV_MANDIR }}"
    @asciidoctor -b manpage -D "{{ DEV_MANDIR }}" man/*.adoc
    @echo "Man pages rendered to {{ DEV_MANDIR }}."

# Clean project artifacts
clean:
    @echo "Cleaning project artifacts..."
    @cargo clean
    @rm -f config.just
    @echo "Cleaned."

# Configure the Justfile
configure *args:
    ./configure {{ args }}

# Check for unused dependencies
deps:
    @cargo machete --with-metadata

# Format Rust code
fmt:
    @cargo +nightly fmt --all

# Check for formating issues in Rust code
fmt-check:
    @cargo +nightly fmt --all --check

# Check Rust code for error
check:
    @cargo check --workspace --all-targets --tests

# Run lints on Rust code
lint:
    @cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run lints on Rust code and apply fixes where possible
lint-fix:
    @cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

# Run Zoi test suite
test:
    @cargo test --all-features -- --test-threads=1

# Bump Zoi version
bump *args:
    @./scripts/bump.sh {{ args }}

# Print this help message
help:
    @just --list
