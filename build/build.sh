#!/bin/bash

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' 

OUTPUT_DIR="./build/compiled"
BINARY_NAME="zoi"
FINAL_BINARY_PATH="$OUTPUT_DIR/$BINARY_NAME"
SRC_BINARY_PATH="./target/release/$BINARY_NAME"

mkdir -p "$OUTPUT_DIR"

COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

echo -e "${CYAN}Building Zoi in release mode for $(uname -s)...${NC}"
echo -e "${CYAN}Commit: $COMMIT${NC}"

if ZOI_COMMIT_HASH="$COMMIT" cargo build --release; then
    echo -e "${GREEN}Cargo build successful.${NC}"

    echo -e "${CYAN}Copying binary to $FINAL_BINARY_PATH...${NC}"
    install -m 755 "$SRC_BINARY_PATH" "$FINAL_BINARY_PATH"

    echo -e "${GREEN}Build complete! Zoi is ready at $FINAL_BINARY_PATH${NC}"
else
    echo -e "${RED}Build failed.${NC}"
    exit 1
fi
