set shell := ["bash", "-c"]

set dotenv-load
set dotenv-filename := "config.just"

PREFIX := env("PREFIX", "/usr/local")
OS_NAME := env("OS_NAME", shell("uname -s | tr '[:upper:]' '[:lower:]'"))
ARCH_NAME := env("ARCH_NAME", shell("uname -m"))
SHELL_NAME := env("SHELL_NAME", shell("basename $SHELL"))
WITH_BIN := env("WITH_BIN", "both")

BINDIR := env("BINDIR", PREFIX + "/bin")
NAME := "zoi"

MINI_NAME := "zoi-mini"
COMMIT_HASH := shell("git rev-parse --short=10 HEAD")
IS_WINDOWS := if OS_NAME == "windows" { "1" } else { "0" }
EXE_EXT := if IS_WINDOWS == "1" { ".exe" } else { "" }
SRC_BIN := "target/release/" + NAME + EXE_EXT
MINI_SRC_BIN := "target/release/" + MINI_NAME + EXE_EXT

[private]
_is_configured := if path_exists("config.just") == "true" { "true" } else { "false" }

default: all

all: build install setup
    @echo "Done"

build:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Building Zoi targets: {{ WITH_BIN }} in release mode (commit: {{ COMMIT_HASH }})..."
    @export ZOI_COMMIT_HASH={{ COMMIT_HASH }}; \
    if [ "{{ WITH_BIN }}" = "zoi" ]; then \
        cargo build --bin zoi --release; \
    elif [ "{{ WITH_BIN }}" = "zoi-mini" ]; then \
        cargo build --bin zoi-mini --release; \
    else \
        cargo build --bin zoi --bin zoi-mini --release; \
    fi
    @echo "Build complete for {{ OS_NAME }} ({{ ARCH_NAME }})."

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

uninstall:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Uninstalling binaries from {{ BINDIR }}..."
    @rm -f "{{ BINDIR }}/{{ NAME }}{{ EXE_EXT }}"
    @rm -f "{{ BINDIR }}/{{ MINI_NAME }}{{ EXE_EXT }}"
    @echo "Binaries uninstalled."

clean:
    @echo "Cleaning project artifacts..."
    @cargo clean
    @rm -f config.mk config.just
    @echo "Cleaned."

setup:
    @if [ "{{ _is_configured }}" != "true" ]; then echo "Error: Project not configured. Run 'just configure' first."; exit 1; fi
    @echo "Running setup for the '{{ SHELL_NAME }}' shell..."
    @{{ SRC_BIN }} shell {{ SHELL_NAME }}
    @{{ SRC_BIN }} setup
    @echo ""
    @echo "Setup complete. Please restart your shell."

configure *args:
    ./configure {{ args }}

help:
    @just --list
