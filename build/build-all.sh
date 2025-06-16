#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' 

COMMIT=$(git rev-parse --short=10 HEAD)

mkdir -p build/compiled

TARGETS=(
  "linux/amd64/"
  "linux/arm64/"
  "darwin/amd64/"
  "darwin/arm64/"
  "windows/amd64/.exe"
  "windows/arm64/.exe"
)

echo -e "${CYAN}🏗 Starting build process...${NC}"
echo -e "${CYAN}▸ Commit: ${COMMIT}${NC}\n"

for target in "${TARGETS[@]}"; do
  IFS='/' read -ra parts <<< "$target"
  GOOS="${parts[0]}"
  GOARCH="${parts[1]}"
  EXT="${parts[2]:-}"

  OUTPUT="zoi-${GOOS}-${GOARCH}${EXT}"
  LDFLAGS="-s -w -X main.VerCommit=${COMMIT}"

  echo -e "${CYAN}🔧 Building ${OUTPUT}...${NC}"
  
  if ! GOOS=$GOOS GOARCH=$GOARCH go build \
    -ldflags "$LDFLAGS" \
    -o "build/compiled/${OUTPUT}" \
    .; then
    echo -e "${RED}❌ Build failed for ${OUTPUT}${NC}"
    exit 1
  fi

  if [[ "$GOOS" != "windows" ]]; then
    chmod +x "build/compiled/${OUTPUT}"
  fi
done

echo -e "\n${GREEN}✅ All builds completed successfully!${NC}"
echo -e "${CYAN}Output files in ./build/compiled directory:${NC}"
ls -lh build/compiled