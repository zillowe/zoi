#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./scripts/release/packages"
mkdir -p "$OUTPUT_DIR"

TARGETS=("x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu")

for TARGET in "${TARGETS[@]}"; do
  echo -e "${CYAN}📦 Generating DEB package for ${TARGET}...${NC}"
  cargo deb -p zoi-rs --target "$TARGET" --no-build
  mv target/"$TARGET"/debian/*.deb "$OUTPUT_DIR/"
done

echo -e "${GREEN}✅ DEB packages generated successfully.${NC}"
