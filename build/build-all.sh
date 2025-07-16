#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\03-e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"'
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
    x86_64-unknown-linux-gnu)  NAME="zoi-linux-amd64" ;;
    aarch64-unknown-linux-gnu) NAME="zoi-linux-arm64" ;;
    x86_64-apple-darwin)       NAME="zoi-macos-amd64" ;;
    aarch64-apple-darwin)      NAME="zoi-macos-arm64" ;;
    x86_64-pc-windows-gnu)     NAME="zoi-windows-amd64.exe" ;;
    # aarch64-pc-windows-gnu)    NAME="zoi-windows-arm64.exe" ;;
    *)                         NAME="zoi-$target" ;; 
  esac
  
  echo -e "${CYAN}🔧 Building for ${target}...${NC}"

  echo "Adding target with rustup..."
  rustup target add "$target"

  LINKER_ENV=""
  if [[ "$target" == "aarch64-unknown-linux-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc"
  elif [[ "$target" == "x86_64-pc-windows-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc"
  elif [[ "$target" == "aarch64-pc-windows-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_AARCH64_PC_WINDOWS_GNU_LINKER=aarch64-w64-mingw32-gcc"
  fi

  echo "Building with cargo..."
  if ! env $LINKER_ENV ZOI_COMMIT_HASH="$COMMIT" cargo build --target "$target" --release; then
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
ls -lh
