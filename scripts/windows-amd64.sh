#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./scripts/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")
TARGET="x86_64-pc-windows-gnu"
NAME="zoi-windows-amd64.exe"
MINI_NAME="zoi-mini-windows-amd64.exe"

echo -e "${CYAN}🏗 Building Zoi for ${TARGET}...${NC}"
mkdir -p "$OUTPUT_DIR"

rustup target add "$TARGET"

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc

if ! ZOI_COMMIT_HASH="$COMMIT" cargo build --bins --target "$TARGET" --release; then
  echo -e "${RED}❌ Build failed for ${TARGET}${NC}"
  exit 1
fi

install -m 755 "target/${TARGET}/release/zoi.exe" "$OUTPUT_DIR/$NAME"
install -m 755 "target/${TARGET}/release/zoi-mini.exe" "$OUTPUT_DIR/$MINI_NAME"

echo -e "${GREEN}✅ Successfully built ${NAME} and ${MINI_NAME}${NC}"
