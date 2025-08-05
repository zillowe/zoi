#!/bin/bash

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' 

OUTPUT_DIR="./build/release"
BINARY_NAME="zoi"
FINAL_BINARY_NAME="zoi"
FINAL_BINARY_PATH="$OUTPUT_DIR/$FINAL_BINARY_NAME"
SRC_BINARY_PATH="./target/release/$BINARY_NAME"

mkdir -p "$OUTPUT_DIR"

COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

echo -e "${CYAN}Building Zoi release binary for $(uname -s)...${NC}"
echo -e "${CYAN}Commit: $COMMIT${NC}"

if ZOI_COMMIT_HASH="$COMMIT" cargo build --release; then
    echo -e "${GREEN}Cargo build successful.${NC}"
else
    echo -e "${RED}Cargo build failed.${NC}"
    exit 1
fi

echo -e "${CYAN}Stripping release binary for size optimization...${NC}"
if strip "$SRC_BINARY_PATH"; then
    echo -e "${GREEN}Binary stripped successfully.${NC}"
else
    echo -e "${RED}Failed to strip binary. The 'strip' command might not be available.${NC}"
fi

echo -e "${CYAN}Copying final binary to $FINAL_BINARY_PATH...${NC}"
install -m 755 "$SRC_BINARY_PATH" "$FINAL_BINARY_PATH"

echo -e "${GREEN}Release build complete! Zoi is ready at $FINAL_BINARY_PATH${NC}"
