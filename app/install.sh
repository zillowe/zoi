#!/usr/bin/env bash

set -euo pipefail

GITLAB_PROJECT_PATH="Zusty/Zoi"
TAG="latest"

REPO_BASE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${TAG}/downloads"

INSTALL_DIR="${HOME}/.local/bin"
BIN_NAME="zoi"
COMMENT_LINE="# Zoi PATH addition"

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
require_util() {
    command -v "$1" >/dev/null 2>&1 || error "'$1' command is required but not found. Please install it."
}


require_util "curl"
require_util "uname"
require_util "chmod"
require_util "mkdir"
require_util "tar"
require_util "xz"

os=""
arch=""
case "$(uname -s)" in
    Linux*)  os="linux" ;;
    Darwin*) os="darwin" ;;
    MINGW*|CYGWIN*|MSYS*) error "Windows detected. Please use the 'install.ps1' script in PowerShell." ;;
    *)       error "Unsupported Operating System: $(uname -s)" ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch="amd64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)          error "Unsupported Architecture: $(uname -m)" ;;
esac

TARGET_ARCHIVE="zoi-${os}-${arch}.tar.xz"
DOWNLOAD_URL="${REPO_BASE_URL}/${TARGET_ARCHIVE}"
CHECKSUM_URL="${REPO_BASE_URL}/checksums.txt"
INSTALL_PATH="${INSTALL_DIR}/${BIN_NAME}"

TEMP_DIR=$(mktemp -d)
TEMP_ARCHIVE="${TEMP_DIR}/${TARGET_ARCHIVE}"
TEMP_CHECKSUMS="${TEMP_DIR}/checksums.txt"

info "Installing/Updating Zoi for ${os}(${arch})..."
info "Target: ${INSTALL_PATH}"

if [ ! -d "$INSTALL_DIR" ]; then
    info "Creating installation directory: ${INSTALL_DIR}"
    mkdir -p "$INSTALL_DIR" || error "Failed to create directory: ${INSTALL_DIR}"
fi

info "Downloading Zoi from: ${DOWNLOAD_URL}"
if curl --fail --location --progress-bar --output "$TEMP_ARCHIVE" "$DOWNLOAD_URL"; then
    info "Download successful to ${TEMP_ARCHIVE}"
else
    rm -f "$TEMP_ARCHIVE"
    error "Download failed. Please check the URL and your connection."
fi

info "Verifying checksum..."
if ! curl --fail --location --progress-bar --output "$TEMP_CHECKSUMS" "$CHECKSUM_URL"; then
    rm -rf "$TEMP_DIR"
    error "Failed to download checksums file: ${CHECKSUM_URL}"
fi

CHECKSUM_CMD=""
if command -v shasum >/dev/null 2>&1; then
    CHECKSUM_CMD="shasum -a 256"
elif command -v sha256sum >/dev/null 2>&1; then
    CHECKSUM_CMD="sha256sum"
else
    error "'shasum' or 'sha256sum' command is required for verification. Please install it."
fi

expected_hash=$(grep "$TARGET_ARCHIVE" "$TEMP_CHECKSUMS" | awk '{print $1}')
if [ -z "$expected_hash" ]; then
    rm -rf "$TEMP_DIR"
    error "Could not find checksum for '${TARGET_ARCHIVE}' in the checksums file."
fi

actual_hash=$($CHECKSUM_CMD "$TEMP_ARCHIVE" | awk '{print $1}')

if [ "$actual_hash" != "$expected_hash" ]; then
    rm -rf "$TEMP_DIR"
    error "Checksum mismatch! The downloaded file may be corrupt or tampered with."
else
    info "Checksum verified successfully."
fi

if [ -f "$INSTALL_PATH" ]; then
    info "Removing existing binary at ${INSTALL_PATH}..."
    rm "$INSTALL_PATH" || warn "Failed to remove existing binary, proceeding with caution."
fi

info "Extracting binary..."
if tar -xf "$TEMP_ARCHIVE" -C "$TEMP_DIR"; then
    info "Extraction successful."
else
    rm -rf "$TEMP_DIR"
    error "Extraction failed."
fi

EXTRACTED_BINARY="${TEMP_DIR}/zoi"
if [ ! -f "$EXTRACTED_BINARY" ]; then
    rm -rf "$TEMP_DIR"
    error "Could not find 'zoi' executable in the extracted contents."
fi

info "Moving binary to ${INSTALL_PATH}..."
mv "$EXTRACTED_BINARY" "$INSTALL_PATH" || error "Failed to move binary to ${INSTALL_PATH}."

rm -rf "$TEMP_DIR"

info "Making binary executable..."
chmod +x "$INSTALL_PATH" || error "Failed to set execute permission on: ${INSTALL_PATH}"

info "Checking if '${INSTALL_DIR}' is in PATH..."
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    warn "'${INSTALL_DIR}' is not found in your current PATH."
    info "Attempting to add it to your shell profile..."

    PROFILE_FILE=""
    if [ -n "${ZSH_VERSION+x}" ] && [ -n "$ZSH_VERSION" ]; then
        PROFILE_FILE="${ZDOTDIR:-$HOME}/.zshrc"
    elif [ -n "${BASH_VERSION+x}" ] && [ -n "$BASH_VERSION" ]; then
        PROFILE_FILE="$HOME/.bashrc"
    elif [ -f "$HOME/.profile" ]; then
        PROFILE_FILE="$HOME/.profile"
    else
        if [ -f "$HOME/.bash_profile" ]; then
            PROFILE_FILE="$HOME/.bash_profile"
        elif [ -f "$HOME/.zprofile" ]; then
             PROFILE_FILE="$HOME/.zprofile"
        fi
    fi

    if [ -n "$PROFILE_FILE" ] && [ -f "$PROFILE_FILE" ]; then
        info "Detected profile file: $PROFILE_FILE"
        EXPORT_LINE="export PATH=\"\$PATH:${INSTALL_DIR}\"" 
        if ! grep -qF -- "$EXPORT_LINE" "$PROFILE_FILE"; then
            info "Adding PATH update to $PROFILE_FILE..."
            [[ $(tail -c1 "$PROFILE_FILE") ]] && echo "" >> "$PROFILE_FILE"
            echo "" >> "$PROFILE_FILE" 
            echo "$COMMENT_LINE" >> "$PROFILE_FILE"
            echo "$EXPORT_LINE" >> "$PROFILE_FILE"
            info "Successfully updated profile. Please run 'source ${PROFILE_FILE}' or restart your shell."
        else
            info "PATH update line already exists in ${PROFILE_FILE}."
        fi
    else
        warn "Could not automatically detect a suitable shell profile file."
        warn "Please add the following line to your shell configuration file manually:"
        warn "  export PATH=\"\$PATH:${INSTALL_DIR}\""
    fi
else
    info "'${INSTALL_DIR}' is already in your PATH."
fi

echo ""
info "Zoi (${TARGET_ARCHIVE}) installed/updated successfully to: ${INSTALL_PATH}"
info "Run 'zoi --version' in a new shell/terminal tab to verify."
