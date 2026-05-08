#!/usr/bin/env bash

# zm.sh - Zero-install Zoi Mini script
# Usage: curl -fsSL https://zillowe.pages.dev/zm.sh | sh -s -- <package>

set -euo pipefail

info() {
    printf "\033[0;36m[INFO] %s\033[0m\n" "$1"
}
error() {
    printf "\033[0;31m[ERROR] %s\033[0m\n" "$1" >&2
    exit 1
}

GITLAB_PROJECT_ID="71087662"
GITLAB_PROJECT_PATH="Zillowe/Zillwen/Zusty/Zoi"

os=""
arch=""
case "$(uname -s)" in
    Linux*)  os="linux" ;;
    Darwin*) os="macos" ;;
    *)       error "Unsupported OS: $(uname -s)" ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch="amd64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)          error "Unsupported Arch: $(uname -m)" ;;
esac

info "Fetching Zoi Mini for ${os}(${arch})..."
LATEST_TAG=$(curl --silent "https://gitlab.com/api/v4/projects/${GITLAB_PROJECT_ID}/releases" | tr ',' '\n' | grep '"tag_name"' | sed 's/.*"tag_name":"\([^"]*\)".*/\1/' | head -n 1)

if [ -z "$LATEST_TAG" ]; then
    error "Could not fetch the latest release tag."
fi

BIN_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads/zoi-mini-${os}-${arch}"

TEMP_BIN="/tmp/zoi-mini"
info "Downloading from: ${BIN_URL}"
curl --fail --location --progress-bar --output "$TEMP_BIN" "$BIN_URL"
chmod +x "$TEMP_BIN"

info "Executing Zoi Mini..."
"$TEMP_BIN" install "$@"
