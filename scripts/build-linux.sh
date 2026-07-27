#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

OUTPUT_DIR="./scripts/release"
COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

TARGETS=(
  "x86_64-unknown-linux-gnu"
  "aarch64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu"
)

if ! command -v cargo &>/dev/null; then
  echo -e "${RED}❌ 'cargo' is not installed or not in the PATH.${NC}"
  exit 1
fi

if ! command -v clang &>/dev/null; then
  echo -e "${RED}❌ 'clang' is not installed. It is required for bindgen during build.${NC}"
  echo -e "${YELLOW}Please install 'clang' and 'libclang-dev' (Debian/Ubuntu) or 'clang-devel' (Fedora).${NC}"
  exit 1
fi

echo -e "${CYAN}🏗 Starting native Linux and Windows build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"
mkdir -p "$OUTPUT_DIR"

for target in "${TARGETS[@]}"; do
  case "$target" in
  x86_64-unknown-linux-gnu)
    NAME="zoi-linux-amd64"
    MINI_NAME="zoi-mini-linux-amd64"
    DAEMON_NAME="zoid-linux-amd64"
    ;;
  aarch64-unknown-linux-gnu)
    NAME="zoi-linux-arm64"
    MINI_NAME="zoi-mini-linux-arm64"
    DAEMON_NAME="zoid-linux-arm64"
    ;;
  x86_64-pc-windows-gnu)
    NAME="zoi-windows-amd64.exe"
    MINI_NAME="zoi-mini-windows-amd64.exe"
    DAEMON_NAME="" # No daemon for Windows
    ;;
  *)
    NAME="zoi-$target"
    MINI_NAME="zoi-mini-$target"
    DAEMON_NAME="zoid-$target"
    ;;
  esac

  echo -e "${CYAN}🔧 Building for ${target}...${NC}"

  rustup target add "$target"

  LINKER_ENV=""
  OPENSSL_ENV=""
  EXTRA_BINS=""
  if [[ "$target" == "aarch64-unknown-linux-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc"
    OPENSSL_ENV="PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig"
    EXTRA_BINS="-p zoi-daemon"
  elif [[ "$target" == "x86_64-unknown-linux-gnu" ]]; then
    EXTRA_BINS="-p zoi-daemon"
  elif [[ "$target" == "x86_64-pc-windows-gnu" ]]; then
    LINKER_ENV="CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc"
  fi

  if ! env $LINKER_ENV "$OPENSSL_ENV" ZOI_COMMIT_HASH="$COMMIT" cargo build -p zoi-rs -p zoi-mini :"$EXTRA_BINS" --target "$target" --release; then
    echo -e "${RED}❌ Build failed for ${target}${NC}"
    exit 1
  fi

  SRC_BINARY="target/${target}/release/zoi"
  MINI_SRC_BINARY="target/${target}/release/zoi-mini"
  DAEMON_SRC_BINARY="target/${target}/release/zoid"

  if [[ "$target" == *"-windows-"* ]]; then
    SRC_BINARY+=".exe"
    MINI_SRC_BINARY+=".exe"
  fi

  install -m 755 "$SRC_BINARY" "$OUTPUT_DIR/$NAME"
  install -m 755 "$MINI_SRC_BINARY" "$OUTPUT_DIR/$MINI_NAME"
  if [[ -n "$DAEMON_NAME" ]]; then
    install -m 755 "$DAEMON_SRC_BINARY" "$OUTPUT_DIR/$DAEMON_NAME"
    echo -e "${GREEN}✅ Successfully built ${NAME}, ${MINI_NAME}, and ${DAEMON_NAME}${NC}\n"
  else
    echo -e "${GREEN}✅ Successfully built ${NAME} and ${MINI_NAME}${NC}\n"
  fi
done

echo -e "\n${GREEN}🎉 All Linux and Windows builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./scripts/release directory:${NC}"
ls -lh "$OUTPUT_DIR"
