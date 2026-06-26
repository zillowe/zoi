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
  echo -e "${CYAN}📦 Generating RPM package for ${TARGET}...${NC}"
  cargo generate-rpm -p zoi-rs --target "$TARGET"
  mv target/"$TARGET"/generate-rpm/*.rpm "$OUTPUT_DIR/"
done

echo -e "${GREEN}✅ RPM packages generated successfully.${NC}"
