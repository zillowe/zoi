#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' 

OUTPUT_DIR="./build/release-all"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGETS=(
  "x86_64-unknown-linux-gnu"  
  "aarch64-unknown-linux-gnu" 
  "x86_64-apple-darwin"       
  "aarch64-apple-darwin"      
  "x86_64-pc-windows-gnu"     
  "aarch64-pc-windows-gnu"    
)

if ! command -v cross &> /dev/null; then
    echo -e "${RED}❌ 'cross' is not installed. Please run 'cargo install cross-rs' first.${NC}"
    exit 1
fi

echo -e "${CYAN}🏗 Starting cross-compilation process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
    x86_64-unknown-linux-gnu)  NAME="zoi-linux-amd64" ;;
    aarch64-unknown-linux-gnu) NAME="zoi-linux-arm64" ;;
    x86_64-apple-darwin)       NAME="zoi-macos-amd64" ;;
    aarch64-apple-darwin)      NAME="zoi-macos-arm64" ;;
    x86_64-pc-windows-gnu)     NAME="zoi-windows-amd64.exe" ;;
    aarch64-pc-windows-gnu)    NAME="zoi-windows-arm64.exe" ;;
    *)                         NAME="zoi-$target" ;; 
  esac
  
  echo -e "${CYAN}🔧 Building for ${target}...${NC}"

  if ! ZOI_COMMIT_HASH="$COMMIT" cross build --target "$target" --release; then
    echo -e "${RED}❌ Build failed for ${target}${NC}"
    exit 1
  fi
  
  SRC_BINARY="target/${target}/release/zoi"
  if [[ "$target" == *"-windows-"* ]]; then
      SRC_BINARY+=".exe"
  fi
  
  install -m 755 "$SRC_BINARY" "$OUTPUT_DIR/$NAME"
  
  echo -e "${GREEN}✅ Successfully built ${NAME}${NC}\n"
done

echo -e "\n${GREEN}🎉 All builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./build/release-all directory:${NC}"
ls -lh "$OUTPUT_DIR"
