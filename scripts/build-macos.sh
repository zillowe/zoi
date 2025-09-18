#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' 

OUTPUT_DIR="./scripts/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGETS=(
  "x86_64-apple-darwin"
  "aarch64-apple-darwin" 
)

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
    echo -e "${RED}Please ensure the runner has Rust/Cargo installed (e.g. via rustup or Homebrew).${NC}"
    exit 1
fi

echo -e "${CYAN}🏗 Starting native macOS build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
    x86_64-apple-darwin)  NAME="zoi-macos-amd64" ;;
    aarch64-apple-darwin) NAME="zoi-macos-arm64" ;;
    *)                    NAME="zoi-$target" ;; 
  esac
  
  echo -e "${CYAN}🔧 Natively building for ${target}...${NC}"

  rustup target add "$target"

  if ! ZOI_COMMIT_HASH="$COMMIT" cargo build --bin zoi --target "$target" --release; then
    echo -e "${RED}❌ Build failed for ${target}${NC}"
    exit 1
  fi
  
  SRC_BINARY="target/${target}/release/zoi"
  
  install -m 755 "$SRC_BINARY" "$OUTPUT_DIR/$NAME"
  
  echo -e "${GREEN}✅ Successfully built ${NAME}${NC}\n"
done

echo -e "\n${GREEN}🎉 All macOS builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./scripts/release directory:${NC}"
ls -lh "$OUTPUT_DIR"
