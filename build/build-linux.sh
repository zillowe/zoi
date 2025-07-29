#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' 

OUTPUT_DIR="./build/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGETS=(
  # "x86_64-unknown-linux-gnu"  
  # "aarch64-unknown-linux-gnu"
  # "x86_64-pc-windows-gnu"
  "x86_64-unknown-freebsd"
  "aarch64-unknown-freebsd"
  "x86_64-unknown-openbsd"
  "aarch64-unknown-openbsd"
)

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
    exit 1
fi

echo -e "${CYAN}🏗 Starting native Linux and Windows build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
    # x86_64-unknown-linux-gnu)  NAME="zoi-linux-amd64" ;;
    # aarch64-unknown-linux-gnu) NAME="zoi-linux-arm64" ;;
    x86_64-unknown-freebsd)    NAME="zoi-freebsd-amd64" ;;
    aarch64-unknown-freebsd)    NAME="zoi-freebsd-arm64" ;;
    x86_64-unknown-openbsd)    NAME="zoi-openbsd-amd64" ;;
    aarch64-unknown-openbsd)    NAME="zoi-openbsd-arm64" ;;
    # x86_64-pc-windows-gnu)     NAME="zoi-windows-amd64.exe" ;;
    *)                         NAME="zoi-$target" ;; 
  esac
  
  echo -e "${CYAN}🔧 Building for ${target}...${NC}"

  rustup target add "$target"

  LINKER_ENV=""
  OPENSSL_ENV=""
  if [[ "$target" == "aarch64-unknown-linux-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc"
    OPENSSL_ENV="PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig"

  elif [[ "$target" == "x86_64-pc-windows-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc"
  fi

  if [[ "$target" == "x86_64-unknown-freebsd" ]]; then
    export CC_x86_64_unknown_freebsd="clang --sysroot=/usr/local/freebsd-sysroot --target=x86_64-unknown-freebsd"
    export CXX_x86_64_unknown_freebsd="clang++ --sysroot=/usr/local/freebsd-sysroot --target=x86_64-unknown-freebsd"
    export AR_x86_64_unknown_freebsd=llvm-ar
    export CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER=clang
    export CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_RUSTFLAGS="-C link-arg=--sysroot=/usr/local/freebsd-sysroot -C link-arg=--target=x86_64-unknown-freebsd"
  elif [[ "$target" == "aarch64-unknown-freebsd" ]]; then
    export CC_aarch64_unknown_freebsd="clang --sysroot=/usr/local/freebsd-sysroot-arm64 --target=aarch64-unknown-freebsd"
    export CXX_aarch64_unknown_freebsd="clang++ --sysroot=/usr/local/freebsd-sysroot-arm64 --target=aarch64-unknown-freebsd"
    export AR_aarch64_unknown_freebsd=llvm-ar
    export CARGO_TARGET_AARCH64_UNKNOWN_FREEBSD_LINKER=clang
    export CARGO_TARGET_AARCH64_UNKNOWN_FREEBSD_RUSTFLAGS="-C link-arg=--sysroot=/usr/local/freebsd-sysroot-arm64 -C link-arg=--target=aarch64-unknown-freebsd"
  elif [[ "$target" == "x86_64-unknown-openbsd" ]]; then
    export CC_x86_64_unknown_openbsd=clang
    export AR_x86_64_unknown_openbsd=llvm-ar
    export CARGO_TARGET_X86_64_UNKNOWN_OPENBSD_LINKER=clang
  elif [[ "$target" == "aarch64-unknown-openbsd" ]]; then
    export CC_aarch64_unknown_openbsd=clang
    export AR_aarch64_unknown_openbsd=llvm-ar
    export CARGO_TARGET_AARCH64_UNKNOWN_OPENBSD_LINKER=clang
  fi

  if ! env $LINKER_ENV $OPENSSL_ENV ZOI_COMMIT_HASH="$COMMIT" cargo build --target "$target" --release; then
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

echo -e "\n${GREEN}🎉 All Linux and Windows builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./build/release directory:${NC}"
ls -lh "$OUTPUT_DIR"
