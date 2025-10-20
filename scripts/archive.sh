#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

COMPILED_DIR="./scripts/release"
ARCHIVE_DIR="./scripts/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"
CHECKSUM_SHA256_FILE="${ARCHIVE_DIR}/checksums-256.txt"
GITLAB_PROJECT_PATH="Zillowe/Zillwen/Zusty/Zoi"
PUBLIC_KEY_URL="https://zillowe.pages.dev/keys/zillowe-main.asc"

function check_command() {
    if ! command -v "$1" &> /dev/null;
 then
        echo -e "${RED}Error: '$1' command is not found.${NC}"
        echo -e "${YELLOW}Please install it and ensure it's in your PATH.${NC}"
        exit 1
    fi
}

function sign_file() {
    local file_to_sign=$1
    echo -e "${CYAN}  -> Signing ${file_to_sign}...${NC}"
    echo "${GPG_PASSPHRASE_B32}" | base32 -d | gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 --armor --detach-sign "$file_to_sign"
}

check_command "7z"
check_command "zstd"
check_command "curl"
check_command "jq"
check_command "gpg"

if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    exit 1
fi

rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"

echo -e "${CYAN}Fetching and importing public key...${NC}"
curl -sL "$PUBLIC_KEY_URL" | gpg --import

echo -e "${CYAN}Fetching the latest release tag from GitLab API...${NC}"

if [ -n "${CI_PROJECT_ID:-}" ]; then
    PROJECT_IDENTIFIER="$CI_PROJECT_ID"
else
    PROJECT_IDENTIFIER="${GITLAB_PROJECT_PATH//\/\%2F}"
fi

LATEST_TAG=""
API_URL="https://gitlab.com/api/v4/projects/${PROJECT_IDENTIFIER}/releases"

echo -e "${CYAN}Trying API URL: ${API_URL}${NC}"

if RESPONSE=$(curl --silent --show-error --fail "$API_URL" 2>&1); then
    if [ -n "$RESPONSE" ] && [ "$RESPONSE" != "[]" ]; then
        LATEST_TAG=$(echo "$RESPONSE" | jq -r '.[0].tag_name // empty' 2>/dev/null || echo "")
    fi
else
    echo -e "${YELLOW}API call failed: $RESPONSE${NC}"
fi

echo -e "${CYAN}📦 Starting archival process...${NC}"

for binary_path in "$COMPILED_DIR"/*; do
    filename=$(basename "$binary_path")
    final_binary_name="zoi"
    [[ "$filename" == *".exe" ]] && final_binary_name="zoi.exe"

    TMP_ARCHIVE_DIR=$(mktemp -d)
    cp "$binary_path" "${TMP_ARCHIVE_DIR}/${final_binary_name}"

    archive_basename=${filename%.exe}

    echo -e "${CYAN}  -> Archiving ${filename}...${NC}"
    
    if [[ "$filename" == *"windows"* ]]; then
        (cd "$TMP_ARCHIVE_DIR" && 7z a -tzip -mx=9 "${archive_basename}.zip" "$final_binary_name" >/dev/null)
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.zip" "${ARCHIVE_DIR}/"
        sign_file "${ARCHIVE_DIR}/${archive_basename}.zip"
    else
        (cd "$TMP_ARCHIVE_DIR" && tar -cf "${archive_basename}.tar" "$final_binary_name")
        zstd -T0 "${TMP_ARCHIVE_DIR}/${archive_basename}.tar"
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.tar.zst" "${ARCHIVE_DIR}/"
        sign_file "${ARCHIVE_DIR}/${archive_basename}.tar.zst"
    fi

    rm -rf "$TMP_ARCHIVE_DIR"
done

echo -e "${CYAN}🔐 Generating sha512 checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums.txt" -not -name "*.asc" -exec sha512sum {} +
) > "$CHECKSUM_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha512sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >> "$CHECKSUM_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

echo -e "${CYAN}🔐 Generating sha256 checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums-sha256.txt" -not -name "*.asc" -exec sha256sum {} +
) > "$CHECKSUM_SHA256_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating sha256 checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha256sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >> "$CHECKSUM_SHA256_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

sign_file "$CHECKSUM_FILE"
sign_file "$CHECKSUM_SHA256_FILE"

echo -e "\n${GREEN}✅ Archiving and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"
