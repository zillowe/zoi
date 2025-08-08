#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

COMPILED_DIR="./build/release"
ARCHIVE_DIR="./build/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"
CHECKSUM_SHA256_FILE="${ARCHIVE_DIR}/checksums-256.txt"
GITLAB_PROJECT_PATH="Zillowe/Zillwen/Zusty/Zoi"

function check_command() {
    if ! command -v "$1" &> /dev/null;
 then
        echo -e "${RED}Error: '$1' command is not found.${NC}"
        echo -e "${YELLOW}Please install it and ensure it's in your PATH.${NC}"
        exit 1
    fi
}

check_command "7z"
check_command "zstd"
check_command "bsdiff"
check_command "curl"
check_command "jq"

if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    exit 1
fi

rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"

echo -e "${CYAN}Fetching the latest release tag from GitLab API...${NC}"

if [ -n "${CI_PROJECT_ID:-}" ]; then
    PROJECT_IDENTIFIER="$CI_PROJECT_ID"
else
    PROJECT_IDENTIFIER="${GITLAB_PROJECT_PATH//\/\%2F}"
fi

LATEST_TAG=""
API_URL="https://gitlab.com/api/v4/projects/${PROJECT_IDENTIFIER}/releases"

echo -e "${CYAN}Trying API URL: ${API_URL}${NC}"

if RESPONSE=$(curl --silent --show-error --fail "$API_URL" 2>&1); then
    if [ -n "$RESPONSE" ] && [ "$RESPONSE" != "[]" ]; then
        LATEST_TAG=$(echo "$RESPONSE" | jq -r '.[0].tag_name // empty' 2>/dev/null || echo "")
    fi
else
    echo -e "${YELLOW}API call failed: $RESPONSE${NC}"
fi

echo -e "${CYAN}🔐 Generating sha512 checksums for raw binaries...${NC}"
rm -f "${ARCHIVE_DIR}/checksums-bin.txt"

(
  cd "$COMPILED_DIR" || exit 1
  for f in *; do
    final_name="zoi"
    [[ "$f" == *".exe" ]] && final_name="zoi.exe"
    sha512sum "$f" | awk -v name="$final_name" '{print $1 "  " name}'
  done
) > "${ARCHIVE_DIR}/checksums-bin.txt"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    IFS='-' read -ra parts <<< "$CI_COMMIT_TAG"
    num_parts=${#parts[@]}
    
    version_num=""
    if [ $num_parts -gt 0 ]; then
        version_num=${parts[$num_parts-1]}
    fi

    if [ $num_parts -gt 2 ]; then
        prerelease=$(echo "${parts[1]}" | tr '[:upper:]' '[:lower:]')
        VERSION="${version_num}-${prerelease}"
    else
        VERSION="$version_num"
    fi

    echo -e "${CYAN}Adding versioned checksums for version ${VERSION}...${NC}"
    (
      cd "$COMPILED_DIR" || exit 1
      for f in *; do
        os_arch_part=$(basename "$f" .exe)
        os_arch_part=${os_arch_part#zoi-}
        versioned_name="zoi-${os_arch_part}-v${VERSION}"
        sha512sum "$f" | awk -v name="$versioned_name" '{print $1 "  " name}'
      done
    ) >> "${ARCHIVE_DIR}/checksums-bin.txt"
fi

if [ -n "$LATEST_TAG" ]; then
    echo -e "${CYAN}Downloading and appending checksums from latest release ${LATEST_TAG}...${NC}"
    OLD_CHECKSUMS_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads/checksums-bin.txt"
    TEMP_OLD_CHECKSUMS=$(mktemp)
    if curl --fail -sL -o "$TEMP_OLD_CHECKSUMS" "$OLD_CHECKSUMS_URL"; then
        cat "$TEMP_OLD_CHECKSUMS" >> "${ARCHIVE_DIR}/checksums-bin.txt"
        sort -u -k2,2 -o "${ARCHIVE_DIR}/checksums-bin.txt" "${ARCHIVE_DIR}/checksums-bin.txt"
        echo -e "${GREEN}Successfully appended and deduplicated old checksums.${NC}"
    else
        echo -e "${YELLOW}Could not download old checksums-bin.txt. Patching from older versions might fail.${NC}"
    fi
    rm -f "$TEMP_OLD_CHECKSUMS"
else
    echo -e "${YELLOW}No latest tag found, skipping checksum accumulation. This is normal for a first release.${NC}"
fi

echo -e "${CYAN}📦 Starting archival process...${NC}"

for binary_path in "$COMPILED_DIR"/*; do
    filename=$(basename "$binary_path")
    final_binary_name="zoi"
    [[ "$filename" == *".exe" ]] && final_binary_name="zoi.exe"

    TMP_ARCHIVE_DIR=$(mktemp -d)
    cp "$binary_path" "${TMP_ARCHIVE_DIR}/${final_binary_name}"

    archive_basename=${filename%.exe}

    echo -e "${CYAN}  -> Archiving ${filename}...${NC}"
    
    if [[ "$filename" == *"windows"* ]]; then
        (cd "$TMP_ARCHIVE_DIR" && 7z a -tzip -mx=9 "${archive_basename}.zip" "$final_binary_name" >/dev/null)
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.zip" "${ARCHIVE_DIR}/"
    else
        (cd "$TMP_ARCHIVE_DIR" && tar -cf "${archive_basename}.tar" "$final_binary_name")
        zstd -T0 "${TMP_ARCHIVE_DIR}/${archive_basename}.tar"
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.tar.zst" "${ARCHIVE_DIR}/"
    fi

    rm -rf "$TMP_ARCHIVE_DIR"
done

echo -e "${CYAN}🔗 Generating binary diffs...${NC}"

if [ -z "$LATEST_TAG" ]; then
    echo -e "${YELLOW}Could not fetch the latest release tag. This might be because:${NC}"
    echo -e "${YELLOW}  - No releases exist yet${NC}"
    echo -e "${YELLOW}  - API requires authentication${NC}"
    echo -e "${YELLOW}  - Network connectivity issues${NC}"
    echo -e "${YELLOW}Skipping diff generation.${NC}"
else
    echo -e "${CYAN}Latest tag found: ${LATEST_TAG}${NC}"
    
    for new_binary_path in "$COMPILED_DIR"/*; do
        filename=$(basename "$new_binary_path")
        if [[ "$filename" == *"windows"* ]]; then
            archive_name="${filename%.exe}.zip"
            old_bin_name="zoi.exe"
        else
            archive_name="${filename}.tar.zst"
            old_bin_name="zoi"
        fi

        OLD_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${LATEST_TAG}/downloads/${archive_name}"
        OLD_ARCHIVE_TMP=$(mktemp)
        OLD_EXTRACT_DIR=$(mktemp -d)

        echo -e "  -> Downloading old archive for ${filename} from ${LATEST_TAG}..."
        if curl --fail -sL -o "$OLD_ARCHIVE_TMP" "$OLD_ARCHIVE_URL"; then
            echo -e "  -> Extracting old archive..."
            if [[ "$archive_name" == *.zip ]]; then
                7z x -y -o"$OLD_EXTRACT_DIR" "$OLD_ARCHIVE_TMP" >/dev/null
            else
                tar --use-compress-program=zstd -xf "$OLD_ARCHIVE_TMP" -C "$OLD_EXTRACT_DIR"
            fi

            OLD_BINARY_PATH="${OLD_EXTRACT_DIR}/${old_bin_name}"
            if [[ ! -f "$OLD_BINARY_PATH" ]]; then
                echo -e "${YELLOW}  -> Could not locate old binary (${old_bin_name}) inside archive. Skipping patch for ${filename}.${NC}"
            else
                PATCH_FILE="${ARCHIVE_DIR}/${filename}.patch"
                echo -e "  -> Creating patch (old binary -> new binary) for ${filename}..."
                bsdiff "$OLD_BINARY_PATH" "$new_binary_path" "$PATCH_FILE"
            fi
        else
            echo -e "${YELLOW}  -> Could not download old archive for ${filename}. Skipping patch.${NC}"
        fi
        rm -rf "$OLD_ARCHIVE_TMP" "$OLD_EXTRACT_DIR"
    done
fi

echo -e "${CYAN}🔐 Generating sha512 checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums.txt" -exec sha512sum {} +
) > "$CHECKSUM_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha512sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >> "$CHECKSUM_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

echo -e "${CYAN}🔐 Generating sha256 checksums...${NC}"
(
  cd "$ARCHIVE_DIR" || exit 1
  find . -maxdepth 1 -type f -not -name "checksums-sha256.txt" -exec sha256sum {} +
) > "$CHECKSUM_SHA256_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating sha256 checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha256sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >> "$CHECKSUM_SHA256_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

echo -e "\n${GREEN}✅ Archiving, diffing, and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"
