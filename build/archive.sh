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
check_command "curl"
check_command "jq"

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

# echo -e "${CYAN}🔗 Generating binary diffs...${NC}"
#
# echo -e "${CYAN}Fetching the latest release tag from GitLab API...${NC}"
#
# if [ -n "${CI_PROJECT_ID:-}" ]; then
#     PROJECT_IDENTIFIER="$CI_PROJECT_ID"
# else
#     PROJECT_IDENTIFIER="${GITLAB_PROJECT_PATH//\//%2F}"
# fi
#
# LATEST_TAG=""
# API_URL="https://gitlab.com/api/v4/projects/${PROJECT_IDENTIFIER}/releases"
#
# echo -e "${CYAN}Trying API URL: ${API_URL}${NC}"
#
# if RESPONSE=$(curl --silent --show-error --fail "$API_URL" 2>&1); then
#     if [ -n "$RESPONSE" ] && [ "$RESPONSE" != "[]" ]; then
#         LATEST_TAG=$(echo "$RESPONSE" | jq -r '.[0].tag_name // empty' 2>/dev/null || echo "")
#     fi
# else
#     echo -e "${YELLOW}API call failed: $RESPONSE${NC}"
# fi
#
#
# if [ -z "$LATEST_TAG" ]; then
#     echo -e "${YELLOW}Could not fetch the latest release tag. This might be because:${NC}"
#     echo -e "${YELLOW}  - No releases exist yet${NC}"
#     echo -e "${YELLOW}  - API requires authentication${NC}"
#     echo -e "${YELLOW}  - Network connectivity issues${NC}"
#     echo -e "${YELLOW}Skipping diff generation.${NC}"
# else
#     echo -e "${CYAN}Latest tag found: ${LATEST_TAG}${NC}"
#
#     for new_binary_path in "$COMPILED_DIR"/*; do
#         filename=$(basename "$new_binary_path")
#         if [[ "$filename" == *"windows"* ]]; then
#             archive_name="${filename%.exe}.zip"
#         else
#             archive_name="${filename}.tar.zst"
#         fi
#
#         OLD_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads/${archive_name}"
#         OLD_ARCHIVE_TMP=$(mktemp)
#         OLD_BINARY_TMP=$(mktemp)
#
#         echo -e "  -> Downloading old archive for ${filename} from ${LATEST_TAG}..."
#         if curl --fail -sL -o "$OLD_ARCHIVE_TMP" "$OLD_ARCHIVE_URL"; then
#             if [[ "$filename" == *"windows"* ]]; then
#                 unzip -j "$OLD_ARCHIVE_TMP" -d "$(dirname "$OLD_BINARY_TMP")" >/dev/null
#                 mv "$(dirname "$OLD_BINARY_TMP")/zoi.exe" "$OLD_BINARY_TMP"
#             else
#                 OLD_TAR_TMP=$(mktemp)
#                 zstd -d "$OLD_ARCHIVE_TMP" -o "$OLD_TAR_TMP"
#                 tar -xf "$OLD_TAR_TMP" -C "$(dirname "$OLD_BINARY_TMP")" --strip-components=0
#                 mv "$(dirname "$OLD_BINARY_TMP")/zoi" "$OLD_BINARY_TMP"
#                 rm -f "$OLD_TAR_TMP"
#             fi
#
#             PATCH_FILE="${ARCHIVE_DIR}/${filename}.patch"
#             echo -e "  -> Creating patch for ${filename}..."
#             bsdiff "$OLD_BINARY_TMP" "$new_binary_path" "$PATCH_FILE"
#         else
#             echo -e "${YELLOW}  -> Could not download old archive for ${filename}. Skipping patch.${NC}"
#         fi
#         rm -f "$OLD_ARCHIVE_TMP" "$OLD_BINARY_TMP"
#     done
# fi

echo -e "${CYAN}🔐 Generating checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums.txt" -exec sha512sum {} +
) > "$CHECKSUM_FILE"

echo -e "\n${GREEN}✅ Archiving, diffing, and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"
