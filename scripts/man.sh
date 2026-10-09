#!/usr/bin/env bash

# Renders the handwritten AsciiDoc man pages in `man/` into roff.
#
# The rendered output is platform independent: AsciiDoc to manpage is a pure
# text transformation with no toolchain or target dependency, so one render is
# shared by every release target rather than being repeated per platform.
#
# Output lands in `scripts/release/man/` and is picked up by `archive.sh`,
# which copies it into the release assets, signs each page, and folds it into
# `checksums.txt`. Package definitions then download the rendered page instead
# of rendering it themselves, which keeps a Ruby toolchain out of the build
# dependency closure of every system that installs Zoi.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0.36m'
YELLOW='\033[1;33m'
NC='\033[0m'

SOURCE_DIR="./man"
OUTPUT_DIR="./scripts/release/man"

if ! command -v asciidoctor &>/dev/null; then
    echo -e "${RED}Error: 'asciidoctor' not found.${NC}"
    echo -e "${YELLOW}Install it (Debian/Ubuntu: 'asciidoctor', Fedora/RHEL:${NC}"
    echo -e "${YELLOW}'rubygem-asciidoctor', Arch: 'asciidoctor') and try again.${NC}"
    exit 1
fi

if [ ! -d "$SOURCE_DIR" ]; then
    echo -e "${RED}Error: man page source directory '${SOURCE_DIR}' not found.${NC}"
    exit 1
fi

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo -e "${CYAN}Rendering man pages from ${SOURCE_DIR}...${NC}"

# A malformed page must fail the release rather than ship a truncated man page,
# so a non-zero exit from asciidoctor propagates through `set -e` and aborts
# `archive.sh` before anything is signed or published.
asciidoctor -b manpage -D "$OUTPUT_DIR" "$SOURCE_DIR"/*.adoc

# The rendered set is asserted here rather than assumed, because a page that
# silently fails to render would otherwise be published as a missing download
# that only shows up as `man zoid` finding nothing on a user's machine.
EXPECTED=("zoi.1" "zoi-rs.3" "zoi-lua.5" "zoid.8")
MISSING=()
for page in "${EXPECTED[@]}"; do
    if [ ! -s "${OUTPUT_DIR}/${page}" ]; then
        MISSING+=("$page")
    fi
done

if [ ${#MISSING[@]} -gt 0 ]; then
    echo -e "${RED}Error: expected man pages were not rendered:${NC}"
    for page in "${MISSING[@]}"; do
        echo -e "${RED}  - ${page}${NC}"
    done
    exit 1
fi

echo -e "${GREEN}Rendered ${#EXPECTED[@]} man pages to ${OUTPUT_DIR}:${NC}"
ls -lh "$OUTPUT_DIR"
