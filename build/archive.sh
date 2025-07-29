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


if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    echo -e "${CYAN}Hint: Run ./build-all.sh first to cross-compile the binaries.${NC}"
    exit 1
fi

if ! command -v 7z &> /dev/null; then
    echo -e "${RED}Error: '7z' command is not found.${NC}"
    echo -e "${YELLOW}Please install 7-Zip (e.g. 'p7zip-full' on Debian/Ubuntu, 'p7zip' on Arch) and ensure it's in your PATH.${NC}"
    exit 1
fi

if ! command -v zstd &> /dev/null; then
    echo -e "${RED}Error: 'zstd' command is not found.${NC}"
    echo -e "${YELLOW}Please install zstd (e.g. 'zstd' on Debian/Ubuntu, 'zstd' on Arch) and ensure it's in your PATH.${NC}"
    exit 1
fi


rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"

if ! touch "${ARCHIVE_DIR}/.test" 2>/dev/null; then
    echo -e "${RED}Error: No write permission for archive directory '${ARCHIVE_DIR}'.${NC}"
    exit 1
fi
rm -f "${ARCHIVE_DIR}/.test"


echo -e "${CYAN}📦 Starting archival process...${NC}"

for binary_path in "$COMPILED_DIR"/*; do
    filename=$(basename "$binary_path")
    
    final_binary_name="zoi"
    if [[ "$filename" == *".exe" ]]; then
        final_binary_name="zoi.exe"
    fi

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


echo -e "${CYAN}🔐 Generating checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums.txt" -exec sha512sum {} +
) > "$CHECKSUM_FILE"

echo -e "\n${GREEN}✅ Archiving and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"