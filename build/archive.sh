#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

COMPILED_DIR="./build/release"
ARCHIVE_DIR="./build/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"
GITLAB_PROJECT_PATH="Zillowe/Zillwen/Zusty/Zoi"

function check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}Error: '$1' command is not found.${NC}"
        echo -e "${YELLOW}Please install it and ensure it's in your PATH.${NC}"
        exit 1
    fi
}

check_command "7z"
check_command "zstd"
check_command "bsdiff"
check_command "glab"
check_command "curl"

if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    exit 1
fi

rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"

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
    else
        (cd "$TMP_ARCHIVE_DIR" && tar -cf "${archive_basename}.tar" "$final_binary_name")
        zstd -T0 "${TMP_ARCHIVE_DIR}/${archive_basename}.tar"
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.tar.zst" "${ARCHIVE_DIR}/"
    fi

    rm -rf "$TMP_ARCHIVE_DIR"
done

echo -e "${CYAN}🔗 Generating binary diffs...${NC}"

echo -e "${CYAN}Fetching the latest release tag from GitLab API...${NC}"
LATEST_TAG=$(curl --silent --fail "https://gitlab.com/api/v4/projects/${GITLAB_PROJECT_PATH//\//%2F}/releases" \
    | jq -r '.[0].tag_name // empty' 2>/dev/null)

if [ -z "$LATEST_TAG" ] && command -v jq >/dev/null 2>&1; then
    LATEST_TAG=$(curl --silent --fail "https://gitlab.com/api/v4/projects/${GITLAB_PROJECT_PATH//\//%2F}/releases" \
        | tr ',' '\n' | grep '"tag_name"' | sed 's/.*"tag_name":"\([^"]*\)".*/\1/' | head -n 1)
fi

if [ -z "$LATEST_TAG" ]; then
    echo -e "${YELLOW}Could not fetch the latest release tag. Skipping diff generation.${NC}"
else
    echo -e "${CYAN}Latest tag found: ${LATEST_TAG}${NC}"
    
    for new_binary_path in "$COMPILED_DIR"/*; do
        filename=$(basename "$new_binary_path")
        
        OLD_BINARY_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads/${filename}"
        OLD_BINARY_TMP=$(mktemp)

        echo -e "  -> Downloading old binary for ${filename} from ${LATEST_TAG}..."
        if curl --fail -sL -o "$OLD_BINARY_TMP" "$OLD_BINARY_URL"; then
            PATCH_FILE="${ARCHIVE_DIR}/${filename}.patch"
            echo -e "  -> Creating patch for ${filename}..."
            bsdiff "$OLD_BINARY_TMP" "$new_binary_path" "$PATCH_FILE"
        else
            echo -e "${YELLOW}  -> Could not download old binary for ${filename}. Skipping patch.${NC}"
        fi
        rm -f "$OLD_BINARY_TMP"
    done
fi

echo -e "${CYAN}🔐 Generating checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums.txt" -exec sha512sum {} +
) > "$CHECKSUM_FILE"

echo -e "\n${GREEN}✅ Archiving, diffing, and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"
