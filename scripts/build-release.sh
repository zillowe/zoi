#!/bin/bash

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' 

OUTPUT_DIR="./scripts/release"
BINARY_NAME="zoi"
FINAL_BINARY_NAME="zoi"
FINAL_BINARY_PATH="$OUTPUT_DIR/$FINAL_BINARY_NAME"
SRC_BINARY_PATH="./target/release/$BINARY_NAME"

mkdir -p "$OUTPUT_DIR"

COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

echo -e "${CYAN}Building Zoi release binary for $(uname -s)...${NC}"
echo -e "${CYAN}Commit: $COMMIT${NC}"

if ZOI_COMMIT_HASH="$COMMIT" cargo build --bins --release; then
    echo -e "${GREEN}Cargo build successful.${NC}"
else
    echo -e "${RED}Cargo build failed.${NC}"
    exit 1
fi

echo -e "${CYAN}Stripping release binaries for size optimization...${NC}"
if strip "$SRC_BINARY_PATH"; then
    echo -e "${GREEN}Zoi binary stripped successfully.${NC}"
fi
if strip "./target/release/zoi-mini"; then
    echo -e "${GREEN}Zoi Mini binary stripped successfully.${NC}"
fi

echo -e "${CYAN}Copying final binaries to $OUTPUT_DIR...${NC}"
install -m 755 "$SRC_BINARY_PATH" "$FINAL_BINARY_PATH"
install -m 755 "./target/release/zoi-mini" "$OUTPUT_DIR/zoi-mini"

echo -e "${GREEN}Release build complete! Zoi is ready at $FINAL_BINARY_PATH${NC}"
