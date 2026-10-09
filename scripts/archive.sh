#!/usr/bin/env bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

COMPILED_DIR="./scripts/release"
ARCHIVE_DIR="./scripts/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"
CHECKSUM_SHA256_FILE="${ARCHIVE_DIR}/checksums-256.txt"
GITLAB_PROJECT_PATH="zillowe/zillwen/zusty/zoi"
PUBLIC_KEY_URL="https://zillowe.pages.dev/keys/zillowe-main.asc"

function check_command() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}Error: '$1' command is not found.${NC}"
        echo -e "${YELLOW}Please install it and ensure it's in your PATH.${NC}"
        exit 1
    fi
}

# ZFVM release tags are Symbolic Form B: Branch-Status-Version, as produced
# by `scripts/bump.sh` and consumed by the `zfvm` crate.
#
# The parsing and the SemVer projection here are delegated to `zfvm tag` and
# `zfvm parse`, the CLI shipped by the `zfvm` crate, so the three places that
# must agree - this script, `crates/core/src/upgrade.rs`, and the specification
# - cannot drift apart. Where the projection differs between them, delta
# upgrades silently fall back to full downloads rather than failing loudly.
#
# This is why the fields are never extracted with `cut -d'-'`: `Pre-Alpha`
# contains a hyphen, so a positional split of `Prod-Pre-Alpha-0.1.0` yields
# status `pre` with version `Alpha`, and neither is valid.
ZFVM="${ZFVM:-zfvm}"

function require_zfvm() {
    if ! command -v "$ZFVM" &>/dev/null; then
        echo -e "${RED}Error: '$ZFVM' not found in PATH.${NC}"
        echo -e "${YELLOW}It ships with the zfvm crate: cargo install zfvm-cli${NC}"
        exit 1
    fi
}

# Extracts one field from a release tag. Reads the tag back through `zfvm tag`
# rather than parsing it here.
function zfvm_tag_field() {
    local tag=$1
    local field=$2

    local parsed
    parsed=$("$ZFVM" tag "$tag" 2>/dev/null) || return 1

    case "$field" in
    branch) printf '%s' "$(printf '%s\n' "$parsed" | awk -F'  *' '/^Branch/ {print $2}')" ;;
    status) printf '%s' "$(printf '%s\n' "$parsed" | awk -F'  *' '/^Status/ {print $2}')" ;;
    number) printf '%s' "$(printf '%s\n' "$parsed" | awk -F'  *' '/^Core/ {print $2}')" ;;
    *) printf '%s' "" ;;
    esac
}

# Projects a release tag onto the SemVer string used to name bsdiff patches.
#
# This must stay identical to `zfvm_to_semver` in
# crates/core/src/upgrade.rs, because `zoi upgrade` reconstructs these exact
# patch filenames when it looks for a delta. The status becomes a numeric
# ordinal rather than a readable label: SemVer compares pre-release
# identifiers numerically before lexically, so a readable `-pre-alpha` would
# sort after `-beta` and the comparison would be wrong.
function zfvm_tag_to_semver() {
    local tag=$1
    "$ZFVM" tag "$tag" 2>/dev/null | awk -F'  *' '/^SemVer/ {print $2}'
}

function sign_file() {
    local file_to_sign=$1
    echo -e "${CYAN}  -> Signing ${file_to_sign}...${NC}"
    echo "${GPG_PASSPHRASE_B32}" | base32 -d | gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 --armor --detach-sign "$file_to_sign"
}

check_command "7z"
check_command "zstd"
check_command "curl"
check_command "jq"
check_command "gpg"
check_command "zbsdiff"
check_command "zbspatch"
# Required for ZFVM tag parsing and SemVer projection. `cargo install zfvm-cli`.
require_zfvm

if [ ! -d "$COMPILED_DIR" ]; then
    echo -e "${RED}Error: Compiled directory '${COMPILED_DIR}' not found.${NC}"
    exit 1
fi

rm -rf "$ARCHIVE_DIR"
mkdir -p "$ARCHIVE_DIR"

echo -e "${CYAN}Fetching and importing public key...${NC}"
curl -sL "$PUBLIC_KEY_URL" | gpg --import

echo -e "${CYAN}Fetching the latest release tag from GitLab API...${NC}"

if [ -n "${CI_PROJECT_ID:-}" ]; then
    PROJECT_IDENTIFIER="$CI_PROJECT_ID"
else
    PROJECT_IDENTIFIER="${GITLAB_PROJECT_PATH//\/\%2F/}"
fi

LATEST_TAG=""
API_URL="https://gitlab.com/api/v4/projects/${PROJECT_IDENTIFIER}/releases"

echo -e "${CYAN}Trying API URL: ${API_URL}${NC}"

# A `Releases:` tag on the pipeline is the input for delta generation below.
if [ -n "${CI_COMMIT_TAG:-}" ]; then
    RELEASES_JSON=$(curl --silent --show-error --fail "$API_URL" 2>&1) || RELEASES_JSON=""
    if [ -z "$RELEASES_JSON" ] || [ "$RELEASES_JSON" == "[]" ]; then
        echo -e "${YELLOW}No existing releases found. Delta patches will be skipped.${NC}"
    fi
else
    # Tagless pipelines still need a release tag to build release notes
    # against, so fall back to the most recent release.
    RELEASES_JSON=$(curl --silent --show-error --fail "$API_URL" 2>&1) || RELEASES_JSON=""
    if [ -n "$RELEASES_JSON" ] && [ "$RELEASES_JSON" != "[]" ]; then
        LATEST_TAG=$(echo "$RELEASES_JSON" | jq -r '.[0].tag_name // empty' 2>/dev/null || echo "")
    fi
fi

PREV_TAG=""
if [ -n "${CI_COMMIT_TAG:-}" ]; then
    # Match on the branch only, so a delta patch is produced across status
    # changes (`Prod-Beta-1.2.0` from `Prod-Release-1.1.0`), which is the
    # common upgrade path. Matching on branch+status left `Pre-Alpha` builds
    # with no predecessor to diff against.
    BRANCH_PART=$(zfvm_tag_field "$CI_COMMIT_TAG" branch)
    # - The current tag is excluded: on a re-run of the same tag it would
    # - otherwise be selected as its own predecessor and produce a patch
    # - from a release to itself.
    PREV_TAG=$(echo "$RELEASES_JSON" | jq -r \
        --arg prefix "$BRANCH_PART-" --arg current "$CI_COMMIT_TAG" \
        '[.[] | select(.tag_name | startswith($prefix))
          | select(.tag_name != $current) | .tag_name] | first // empty' \
        2>/dev/null || echo "")
    if [ -n "$PREV_TAG" ]; then
        echo -e "${CYAN}Detected previous tag for prefix ${BRANCH_PART}: ${PREV_TAG}${NC}"
    fi
fi

echo -e "${CYAN}📦 Starting archival process...${NC}"

if [ -d "$COMPILED_DIR/packages" ]; then
    echo -e "${CYAN}  -> Processing .deb and .rpm packages...${NC}"
    for pkg_path in "$COMPILED_DIR/packages"/*; do
        if [ -f "$pkg_path" ]; then
            pkg_filename=$(basename "$pkg_path")
            echo -e "${CYAN}     -> Copying and signing ${pkg_filename}...${NC}"
            cp "$pkg_path" "$ARCHIVE_DIR/"
            sign_file "$ARCHIVE_DIR/$pkg_filename"
        fi
    done
fi

for binary_path in "$COMPILED_DIR"/*; do
    [ -f "$binary_path" ] || continue
    filename=$(basename "$binary_path")

    if [[ "$filename" == "zoi-mini"* ]]; then
        binary_base="zoi-mini"
    elif [[ "$filename" == "zoid"* ]]; then
        binary_base="zoid"
    else
        binary_base="zoi"
    fi

    final_binary_name="$binary_base"
    [[ "$filename" == *".exe" ]] && final_binary_name="${binary_base}.exe"

    TMP_ARCHIVE_DIR=$(mktemp -d)
    cp "$binary_path" "${TMP_ARCHIVE_DIR}/${final_binary_name}"

    archive_basename=${filename%.exe}

    echo -e "${CYAN}  -> Archiving ${filename}...${NC}"

    if [[ "$filename" == *"windows"* ]]; then
        (cd "$TMP_ARCHIVE_DIR" && 7z a -tzip -mx=9 "${archive_basename}.zip" "$final_binary_name" >/dev/null)
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.zip" "${ARCHIVE_DIR}/"
        sign_file "${ARCHIVE_DIR}/${archive_basename}.zip"
    else
        (cd "$TMP_ARCHIVE_DIR" && tar -cf "${archive_basename}.tar" "$final_binary_name")
        zstd -T0 "${TMP_ARCHIVE_DIR}/${archive_basename}.tar"
        mv "${TMP_ARCHIVE_DIR}/${archive_basename}.tar.zst" "${ARCHIVE_DIR}/"
        sign_file "${ARCHIVE_DIR}/${archive_basename}.tar.zst"
    fi

    # Delta patch generation
    # Only generate patches for 'zoi', skipping 'zoi-mini' and 'zoid'
    if [[ "$binary_base" == "zoi" ]] && [ -n "${PREV_TAG:-}" ] && [ -n "${CI_COMMIT_TAG:-}" ]; then
        # Both tags are ZFVM Symbolic Form B. The projection goes through
        # the `zfvm` crate so it matches `zoi upgrade` exactly; see
        # zfvm_tag_to_semver above for why that matters.
        OLD_VERSION=$(zfvm_tag_to_semver "$PREV_TAG")
        CURRENT_VERSION=$(zfvm_tag_to_semver "$CI_COMMIT_TAG")

        if [ -z "$OLD_VERSION" ] || [ -z "$CURRENT_VERSION" ]; then
            echo -e "${YELLOW}Could not project a tag to SemVer, skipping delta patches.${NC}"
            rm -rf "$TMP_ARCHIVE_DIR"
            continue
        fi

        OLD_EXT=".tar.zst"
        [[ "$filename" == *"windows"* ]] && OLD_EXT=".zip"

        OLD_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/releases/${PREV_TAG}/downloads/${archive_basename}${OLD_EXT}"

        OLD_ARCHIVE_FILE="${TMP_ARCHIVE_DIR}/old_archive${OLD_EXT}"
        if curl --fail -sL -o "$OLD_ARCHIVE_FILE" "$OLD_ARCHIVE_URL"; then
            echo -e "${CYAN}  -> Generating bsdiff patch from ${PREV_TAG}...${NC}"
            OLD_BIN_DIR="${TMP_ARCHIVE_DIR}/old_bin"
            mkdir -p "$OLD_BIN_DIR"
            if [[ "$filename" == *"windows"* ]]; then
                unzip -q -o "$OLD_ARCHIVE_FILE" -d "$OLD_BIN_DIR"
            else
                tar -xf "$OLD_ARCHIVE_FILE" -C "$OLD_BIN_DIR" --use-compress-program=zstd
            fi

            OLD_BIN_FILE="$OLD_BIN_DIR/$final_binary_name"
            if [ -f "$OLD_BIN_FILE" ]; then
                BSDIFF_NAME="${archive_basename}.from-v${OLD_VERSION}-to-v${CURRENT_VERSION}.bsdiff"
                zbsdiff "$OLD_BIN_FILE" "$binary_path" "${TMP_ARCHIVE_DIR}/patch.raw"
                zstd -19 -q "${TMP_ARCHIVE_DIR}/patch.raw" -o "${ARCHIVE_DIR}/${BSDIFF_NAME}"
                sign_file "${ARCHIVE_DIR}/${BSDIFF_NAME}"
            fi
        fi
    fi

    rm -rf "$TMP_ARCHIVE_DIR"
done

# Man pages are rendered once by `scripts/man.sh` and published alongside the
# binaries, rather than each package definition rendering the AsciiDoc itself.
#
# They are only reachable from the release assets, so a package that installs
# `zoi` as its own dependency would otherwise have to carry a Ruby toolchain
# in its build closure just to produce a man page. Publishing them here means
# the toolchain is needed once, in CI, and never on a user's machine.
#
# The LICENSE is published for the same reason. A package definition should
# depend only on the release assets it can verify, not on a raw tag tree whose
# contents change the moment a commit lands on the branch.
#
# Both are staged before the checksum pass below so they are covered by
# `checksums.txt` and by its detached signature, which is what lets a package
# verify a downloaded file against the same trust anchor as a binary.
MAN_DIR="./scripts/release/man"

publish_asset() {
    local source=$1
    local filename=$(basename "$source")

    if [ ! -s "$source" ] || [ -s "$ARCHIVE_DIR/$filename" ]; then
        return
    fi

    echo -e "${CYAN}     -> Copying and signing ${filename}...${NC}"
    cp "$source" "$ARCHIVE_DIR/"
    sign_file "$ARCHIVE_DIR/$filename"
}

if [ -d "$MAN_DIR" ]; then
    echo -e "${CYAN}  -> Processing man pages...${NC}"
    for man_path in "$MAN_DIR"/*; do
        [ -f "$man_path" ] || continue
        publish_asset "$man_path"
    done
else
    echo -e "${YELLOW}No man pages found in ${MAN_DIR}; the release will not publish any.${NC}"
    echo -e "${YELLOW}Run './scripts/man.sh' before this script to render them.${NC}"
fi

echo -e "${CYAN}  -> Processing LICENSE...${NC}"
publish_asset "./LICENSE"

echo -e "${CYAN}🔐 Generating sha512 checksums...${NC}"
(
    cd "$ARCHIVE_DIR" || exit 1
    find . -maxdepth 1 -type f -not -name "checksums.txt" \
        -not -name "checksums-256.txt" -not -name "*.asc" \
        -exec sha512sum {} +
) >"$CHECKSUM_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha512sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >>"$CHECKSUM_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

echo -e "${CYAN}🔐 Generating sha256 checksums...${NC}"
(
    cd "$ARCHIVE_DIR" || exit 1
    find . -maxdepth 1 -type f -not -name "checksums-256.txt" \
        -not -name "checksums.txt" -not -name "*.asc" \
        -exec sha256sum {} +
) >"$CHECKSUM_SHA256_FILE"

if [ -n "${CI_COMMIT_TAG:-}" ]; then
    echo -e "${CYAN}🔐 Generating sha256 checksum for source archive ${CI_COMMIT_TAG}...${NC}"
    SOURCE_ARCHIVE_URL="https://gitlab.com/${GITLAB_PROJECT_PATH}/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz"
    SOURCE_ARCHIVE_FILE=$(mktemp)
    if curl --fail -sL -o "$SOURCE_ARCHIVE_FILE" "$SOURCE_ARCHIVE_URL"; then
        sha256sum "$SOURCE_ARCHIVE_FILE" | sed "s|$(basename "$SOURCE_ARCHIVE_FILE")|Zoi-${CI_COMMIT_TAG}.tar.gz|" >>"$CHECKSUM_SHA256_FILE"
    else
        echo -e "${YELLOW}Could not download source archive. Skipping its checksum.${NC}"
    fi
    rm -f "$SOURCE_ARCHIVE_FILE"
fi

sign_file "$CHECKSUM_FILE"
sign_file "$CHECKSUM_SHA256_FILE"

echo -e "\n${GREEN}✅ Archiving and checksum generation complete!${NC}"
echo -e "${CYAN}Output files are in the '${ARCHIVE_DIR}' directory.${NC}"
ls -lh "$ARCHIVE_DIR"
