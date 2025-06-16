#!/usr/bin/env bash
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

REPO_NAME="Zusty/Zoi"

if [ -z "$VERSION_TAG" ] || [ -z "$VERSION_NAME" ]; then
  echo -e "${RED}Error: Missing required arguments.${NC}"
  echo "Usage: ./build/publish.sh <version-tag> \"<version-name>\""
  echo "Example: ./build/publish.sh Prod-Release-3.0.0 \"Prod Release 3.0.0\""
  exit 1
fi

if ! command -v fj &> /dev/null; then
    echo -e "${RED}Error: 'fj' command is not found.${NC}"
    echo -e "${YELLOW}Please install the forgejo-cli and ensure it's in your PATH.${NC}"
    exit 1
fi

echo -e "${YELLOW}Starting Zoi Release Preparation for tag: ${VERSION_TAG}${NC}"

echo -e "\n${CYAN}🗑️  Cleaning up old artifacts...${NC}"
rm -rf "./build/compiled"
rm -rf "./build/archived"
echo -e "${GREEN}✓ Cleanup complete.${NC}"

echo -e "\n${CYAN}🏗️  Running the build script...${NC}"
if ! "./build/build-all.sh"; then
    echo -e "\n${RED}❌ Build process failed.${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Build process finished successfully.${NC}"

echo -e "\n${CYAN}📦 Running the archive script...${NC}"
if ! "./build/archive.sh"; then
    echo -e "\n${RED}❌ Archival process failed.${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Archival process finished successfully.${NC}"

ARCHIVED_DIR="./build/archived"
echo -e "\n${GREEN}✅ Release preparation complete! Artifacts are in '${ARCHIVED_DIR}'.${NC}"

echo -e "\n${YELLOW}Starting Publishing Process...${NC}"

echo -e "\n${CYAN}🚀 Creating new release on Codeberg for tag '${VERSION_TAG}'...${NC}"

if !  fj release create --tag "${VERSION_TAG}" "${VERSION_NAME}"; then
    echo -e "\n${RED}❌ Failed to create release. Does a release for this tag already exist?${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Release created successfully.${NC}"

echo -e "\n${CYAN}⬆️  Uploading assets to the release...${NC}"
ASSET_COUNT=0
for asset in "${ARCHIVED_DIR}"/*; do
    if [ -f "$asset" ]; then
        echo "   - Uploading $(basename "$asset")..."
        if ! fj release asset create "${VERSION_TAG}" "$asset"; then
            echo -e "\n${RED}❌ Failed to upload asset '$(basename "$asset")'.${NC}"
        else
            ASSET_COUNT=$((ASSET_COUNT + 1))
        fi
    fi
done

echo -e "\n${GREEN}✓ Uploaded ${ASSET_COUNT} assets successfully.${NC}"
echo -e "\n${GREEN}✅ Publishing for version ${VERSION_TAG} is complete!${NC}"