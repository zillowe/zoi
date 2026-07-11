set shell := ["bash", "-c"]

set dotenv-load
set dotenv-filename := "config.just"

PREFIX := env("PREFIX", "/usr/local")
OS_NAME := env("OS_NAME", shell("uname -s | tr '[:upper:]' '[:lower:]'"))
ARCH_NAME := env("ARCH_NAME", shell("uname -m"))
WITH_BIN := env("WITH_BIN", "both")

BINDIR := env("BINDIR", PREFIX + "/bin")
DEBUGDIR := env("DEBUGDIR", "dist")
DEV_BINDIR := DEBUGDIR + "/bin"
DEV_MANDIR := DEBUGDIR + "/man"
NAME := "zoi"

MINI_NAME := "zoi-mini"
COMMIT_HASH := shell("git rev-parse --short=10 HEAD")
IS_WINDOWS := if OS_NAME == "windows" { "1" } else { "0" }
EXE_EXT := if IS_WINDOWS == "1" { ".exe" } else { "" }
SRC_BIN := "target/release/" + NAME + EXE_EXT
MINI_SRC_BIN := "target/release/" + MINI_NAME + EXE_EXT
DEBUG_SRC_BIN := "target/debug/" + NAME + EXE_EXT
MINI_DEBUG_SRC_BIN := "target/debug/" + MINI_NAME + EXE_EXT

# Build number file for tracking dev builds on same commit
BUILD_NO_FILE := ".build_no"

[private]
_is_configured := if path_exists("config.just") == "true" { "true" } else { "false" }

[private]
get_build_no:
    @BUILD_NO_FILE=.build_no; LAST_COMMIT=$(cat "$BUILD_NO_FILE.commit" 2>/dev/null || echo ""); BUILD_NO=$(cat "$BUILD_NO_FILE.count" 2>/dev/null || echo 0); COMMIT_HASH=$(git rev-parse --short=10 HEAD); if [ "$LAST_COMMIT" = "$COMMIT_HASH" ]; then NEW_BUILD_NO=$((BUILD_NO + 1)); echo "$NEW_BUILD_NO" > "$BUILD_NO_FILE.count"; echo "$NEW_BUILD_NO"; else echo "$COMMIT_HASH" > "$BUILD_NO_FILE.commit"; echo 1 > "$BUILD_NO_FILE.count"; echo 1; fi

# Alias to `just help`
default: help

# Build Zoi in release mode
build:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v clang >/dev/null 2>&1; then echo "Error: 'clang' is not installed. It is required for bindgen."; exit 1; fi
    @BUILD_NO=$(just get_build_no); \
    echo "Building Zoi targets: {{ WITH_BIN }} in release mode (commit: {{ COMMIT_HASH }}, build: $BUILD_NO)..."; \
    set -a; source .env 2>/dev/null; export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; export ZOI_BUILD_NO=$BUILD_NO; \
    if [ "{{ WITH_BIN }}" = "zoi" ]; then \
        cargo build --bin zoi --release; \
    elif [ "{{ WITH_BIN }}" = "zoi-mini" ]; then \
        cargo build --bin zoi-mini --release; \
    else \
        cargo build --bin zoi --bin zoi-mini --release; \
    fi
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

# Build Zoi in debug mode
dev:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @if ! command -v clang >/dev/null 2>&1; then echo "Error: 'clang' is not installed. It is required for bindgen."; exit 1; fi
    @BUILD_NO=$(just get_build_no); \
    echo "Building Zoi targets: {{ WITH_BIN }} in debug mode (commit: {{ COMMIT_HASH }}, build: $BUILD_NO)..."; \
    set -a; source .env 2>/dev/null; export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; export ZOI_BUILD_NO=$BUILD_NO; \
    if [ "{{ WITH_BIN }}" = "zoi" ]; then \
        cargo build --bin zoi; \
    elif [ "{{ WITH_BIN }}" = "zoi-mini" ]; then \
        cargo build --bin zoi-mini; \
    else \
        cargo build --bin zoi --bin zoi-mini; \
    fi
    @mkdir -p "{{ DEV_BINDIR }}"
    @if [ "{{ WITH_BIN }}" != "zoi-mini" ]; then \
        cp -f "{{ DEBUG_SRC_BIN }}" "{{ DEV_BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
        echo "Zoi (debug) copied to {{ DEV_BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" != "zoi" ]; then \
        cp -f "{{ MINI_DEBUG_SRC_BIN }}" "{{ DEV_BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
        echo "Zoi Mini (debug) copied to {{ DEV_BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
    fi
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

# Install Zoi binary to `PREFIX` or default user's bin location
install:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Installing requested binaries to {{ BINDIR }}..."
    @mkdir -p "{{ BINDIR }}"
    @if [ "{{ WITH_BIN }}" != "zoi-mini" ]; then \
        if [ "{{ IS_WINDOWS }}" = "1" ]; then \
            cp -f "{{ SRC_BIN }}" "{{ BINDIR }}/{{ NAME }}.exe"; \
        else \
            install -m 755 "{{ SRC_BIN }}" "{{ BINDIR }}/{{ NAME }}"; \
        fi; \
        echo "Zoi installed successfully to {{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"; \
    fi
    @if [ "{{ WITH_BIN }}" != "zoi" ]; then \
        if [ "{{ IS_WINDOWS }}" = "1" ]; then \
            cp -f "{{ MINI_SRC_BIN }}" "{{ BINDIR }}/{{ MINI_NAME }}.exe"; \
        else \
            install -m 755 "{{ MINI_SRC_BIN }}" "{{ BINDIR }}/{{ MINI_NAME }}"; \
        fi; \
        echo "Zoi Mini installed successfully to {{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"; \
    fi
    @echo "Make sure '{{ BINDIR }}' is in your PATH."

# Uninstall Zoi binary
uninstall:
    @if [ "{{ _is_configured }}" !290.0= "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Uninstalling binaries from {{ BINDIR }}..."
    @rm -f "{{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"
    @echo "Binaries uninstalled."

# Generate man pages
man:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @mkdir -p "{{ DEV_MANDIR }}"
    @OUT_DIR="{{ DEV_MANDIR }}" {{ DEBUG_SRC_BIN }} generate-manual

# Clean project artifacts
clean:
    @echo "Cleaning project artifacts..."
    @cargo clean
    @rm -f config.mk config.just
    @echo "Cleaned."

# Configure the Justfile
configure *args:
    ./configure {{ args }}

# Print this help message
help:
    @just --list
