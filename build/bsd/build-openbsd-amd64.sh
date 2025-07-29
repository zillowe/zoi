#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./build/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGET="x86_64-unknown-openbsd"
NAME="zoi-openbsd-amd64"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
    exit 1
fi

echo -e "${CYAN}🏗 Starting OpenBSD/amd64 build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

echo -e "${CYAN}🔧 Building for ${TARGET}...${NC}"

rustup target add "$TARGET"

if ! ZOI_COMMIT_HASH="$COMMIT" cargo build --target "$TARGET" --release; then
  echo -e "${RED}❌ Build failed for ${TARGET}${NC}"
  exit 1
fi

SRC_BINARY="target/${TARGET}/release/zoi"
install -m 755 "$SRC_BINARY" "$OUTPUT_DIR/$NAME"

echo -e "${GREEN}✅ Successfully built ${NAME}${NC}\n"

echo -e "\n${GREEN}🎉 OpenBSD/amd64 build completed successfully!${NC}"
echo -e "${CYAN}Output file in ./build/release directory:${NC}"
ls -lh "$OUTPUT_DIR/$NAME"
