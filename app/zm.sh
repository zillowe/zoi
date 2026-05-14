#!/usr/bin/env bash

# zm.sh - Zero-install Zoi Mini script
# Usage: curl -fsSL https://zillowe.pages.dev/zm.sh | sh -s -- i <package>

set -euo pipefail

info() {
    printf "\033[0;36m[INFO] %s\033[0m\n" "$1"
}
error() {
    printf "\033[0;31m[ERROR] %s\033[0m\n" "$1" >&2
    exit 1
}
warn() {
    printf "\033[1;33m[WARN] %s\033[0m\n" "$1"
}

GITLAB_PROJECT_ID="71087662"
GITLAB_PROJECT_PATH="Zillowe/Zillwen/Zusty/Zoi"
PUBLIC_KEY_URL="https://zillowe.pages.dev/keys/zillowe-main.asc"

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

info "Fetching the latest release tag from GitLab API..."
LATEST_TAG=$(curl --silent "https://gitlab.com/api/v4/projects/${GITLAB_PROJECT_ID}/releases" | tr ',' '\n' | grep '"tag_name"' | sed 's/.*"tag_name":"\([^"]*\)".*/\1/' | head -n 1)

if [ -z "$LATEST_TAG" ]; then
    error "Could not fetch the latest release tag."
fi

REPO_BASE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads"
TARGET_ARCHIVE="zoi-mini-${os}-${arch}.tar.zst"
DOWNLOAD_URL="${REPO_BASE_URL}/${TARGET_ARCHIVE}"
SIGNATURE_URL="${DOWNLOAD_URL}.asc"
CHECKSUM_URL="${REPO_BASE_URL}/checksums.txt"

TEMP_DIR=$(mktemp -d)
TEMP_ARCHIVE="${TEMP_DIR}/${TARGET_ARCHIVE}"
TEMP_SIGNATURE="${TEMP_DIR}/${TARGET_ARCHIVE}.asc"
TEMP_CHECKSUMS="${TEMP_DIR}/checksums.txt"
TEMP_PUBKEY="${TEMP_DIR}/pubkey.asc"

trap 'rm -rf "$TEMP_DIR"' EXIT

info "Downloading Zoi Mini for ${os}(${arch})..."
curl --fail --location --progress-bar --output "$TEMP_ARCHIVE" "$DOWNLOAD_URL"

info "Verifying checksum..."
curl --fail --location --silent --output "$TEMP_CHECKSUMS" "$CHECKSUM_URL"

CHECKSUM_CMD=""
if command -v sha512sum >/dev/null 2>&1; then
    CHECKSUM_CMD="sha512sum"
elif command -v shasum >/dev/null 2>&1; then
    CHECKSUM_CMD="shasum -a 512"
else
    error "'sha512sum' or 'shasum' command is required for verification."
fi

expected_hash=$(grep "$TARGET_ARCHIVE" "$TEMP_CHECKSUMS" | awk '{print $1}')
if [ -z "$expected_hash" ]; then
    error "Could not find checksum for '${TARGET_ARCHIVE}'."
fi

actual_hash=$($CHECKSUM_CMD "$TEMP_ARCHIVE" | awk '{print $1}')

if [ "$actual_hash" != "$expected_hash" ]; then
    error "Checksum mismatch!"
else
    info "Checksum verified successfully."
fi

info "Verifying GPG signature..."
if command -v gpg >/dev/null 2>&1; then
    curl --fail --location --silent --output "$TEMP_SIGNATURE" "$SIGNATURE_URL"
    curl --fail --location --silent --output "$TEMP_PUBKEY" "$PUBLIC_KEY_URL"

    if gpg --import "$TEMP_PUBKEY" >/dev/null 2>&1 && gpg --verify "$TEMP_SIGNATURE" "$TEMP_ARCHIVE" >/dev/null 2>&1; then
        info "GPG signature verified successfully."
    else
        error "GPG signature verification failed!"
    fi
else
    warn "GPG not found, skipping signature verification."
fi

info "Extracting binary..."
zstd -dc "$TEMP_ARCHIVE" | tar -xf - -C "$TEMP_DIR"

TEMP_BIN="${TEMP_DIR}/zoi-mini"
if [ ! -f "$TEMP_BIN" ]; then
    error "Could not find 'zoi-mini' executable in the archive."
fi
chmod +x "$TEMP_BIN"

cmd="install"
if [ $# -gt 0 ]; then
    case "$1" in
        install|i|update|up|uninstall|un|list|ls)
            cmd="$1"
            shift
            ;;
    esac
fi

info "Executing Zoi Mini ${cmd}..."
"$TEMP_BIN" "$cmd" "$@"
