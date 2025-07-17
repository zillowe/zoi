#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' 

OUTPUT_DIR="./build/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGETS=(
  "x86_64-unknown-linux-gnu"  
  "aarch64-unknown-linux-gnu" 
)

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
    exit 1
fi

echo -e "${CYAN}🏗 Starting native Linux build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
    x86_64-unknown-linux-gnu)  NAME="zoi-linux-amd64" ;;
    aarch64-unknown-linux-gnu) NAME="zoi-linux-arm64" ;;
    *)                         NAME="zoi-$target" ;; 
  esac
  
  echo -e "${CYAN}🔧 Building for ${target}...${NC}"

  rustup target add "$target"

  LINKER_ENV=""
  OPENSSL_ENV=""
  if [[ "$target" == "aarch64-unknown-linux-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc"
    OPENSSL_ENV="PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_DIR=/usr/lib/aarch64-linux-gnu AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_INCLUDE_DIR=/usr/include/openssl"
  fi

  if ! env $LINKER_ENV $OPENSSL_ENV ZOI_COMMIT_HASH="$COMMIT" cargo build --target "$target" --release; then
    echo -e "${RED}❌ Build failed for ${target}${NC}"
    exit 1
  fi
  
  SRC_BINARY="target/${target}/release/zoi"
  
  install -m 755 "$SRC_BINARY" "$OUTPUT_DIR/$NAME"
  
  echo -e "${GREEN}✅ Successfully built ${NAME}${NC}\n"
done

echo -e "\n${GREEN}🎉 All Linux builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./build/release directory:${NC}"
ls -lh "$OUTPUT_DIR"
