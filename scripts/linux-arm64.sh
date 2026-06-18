#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./scripts/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")
TARGET="aarch64-unknown-linux-gnu"
NAME="zoi-linux-arm64"
MINI_NAME="zoi-mini-linux-arm64"

echo -e "${CYAN}🏗 Building Zoi for ${TARGET}...${NC}"
mkdir -p "$OUTPUT_DIR"

rustup target add "$TARGET"

export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig

if ! ZOI_COMMIT_HASH="$COMMIT" cargo build --bins --target "$TARGET" --release; then
  echo -e "${RED}❌ Build failed for ${TARGET}${NC}"
  exit 1
fi

install -m 755 "target/${TARGET}/release/zoi" "$OUTPUT_DIR/$NAME"
install -m 755 "target/${TARGET}/release/zoi-mini" "$OUTPUT_DIR/$MINI_NAME"

echo -e "${GREEN}✅ Successfully built ${NAME} and ${MINI_NAME}${NC}"
