#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./scripts/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")
TARGET="x86_64-unknown-linux-gnu"
NAME="zoi-linux-amd64"
MINI_NAME="zoi-mini-linux-amd64"
DAEMON_NAME="zoid-linux-amd64"

echo -e "${CYAN}🏗 Building Zoi for ${TARGET}...${NC}"
mkdir -p "$OUTPUT_DIR"

if ! command -v clang &>/dev/null; then
  echo -e "${RED}❌ 'clang' is not installed. It is required for bindgen during build.${NC}"
  exit 1
fi

rustup target add "$TARGET"

if ! ZOI_COMMIT_HASH="$COMMIT" cargo build -p zoi-rs -p zoi-mini -p zoi-daemon --target "$TARGET" --release; then
  echo -e "${RED}❌ Build failed for ${TARGET}${NC}"
  exit 1
fi

install -m 755 "target/${TARGET}/release/zoi" "$OUTPUT_DIR/$NAME"
install -m 755 "target/${TARGET}/release/zoi-mini" "$OUTPUT_DIR/$MINI_NAME"
install -m 755 "target/${TARGET}/release/zoid" "$OUTPUT_DIR/$DAEMON_NAME"

echo -e "${GREEN}✅ Successfully built ${NAME}, ${MINI_NAME}, and ${DAEMON_NAME}${NC}"
