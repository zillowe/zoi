#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

COMPILED_DIR="./build/compiled"
ARCHIVE_DIR="./build/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"

if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    echo -e "${CYAN}Hint: Run ./build/build-all.sh first.${NC}"
    exit 1
fi

rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"
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
        (cd "$TMP_ARCHIVE_DIR" && zip -q "${ARCHIVE_DIR}/${archive_basename}.zip" "$final_binary_name")
    else
        (cd "$TMP_ARCHIVE_DIR" && tar -cJf "${ARCHIVE_DIR}/${archive_basename}.tar.xz" "$final_binary_name")
    fi

    rm -rf "$TMP_ARCHIVE_DIR"
done

echo -e "${CYAN}🔐 Generating checksums...${NC}"
(cd "$ARCHIVE_DIR" && shasum -a 256 ./* > "$CHECKSUM_FILE")


echo -e "\n${GREEN}✅ Archiving and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"