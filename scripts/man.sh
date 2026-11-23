#!/usr/bin/env bash

# Renders the handwritten AsciiDoc man pages in `man/` into roff.
#
# The rendered output is platform independent: AsciiDoc to manpage is a pure
# text transformation with no toolchain or target dependency, so one render is
# shared by every release target rather than being repeated per platform.
#
# Each page's footer carries the Zoi version (from `Cargo.toml`) next to its
# source name, and the date the page itself last changed: its last commit
# date, or in a release source tarball the date recorded in
# `man/PAGE-DATES` (see `scripts/source-tarball.sh`). Rendering the same
# release again gives the same pages.
#
# Usage: scripts/man.sh [OUTPUT_DIR]   (default: scripts/release/man)
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
OUTPUT_DIR="${1:-./scripts/release/man}"

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

# The workspace version, `[workspace.package] version = "..."`.
VERSION=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n 1)
if [ -z "$VERSION" ]; then
    echo -e "${RED}Error: could not read the version from Cargo.toml.${NC}"
    exit 1
fi

# The date a page last changed, as YYYY-MM-DD: its last commit, or in a
# release source tarball (no `.git`) the date recorded in `man/PAGE-DATES`
# when the tarball was made, or else the file's modification time.
page_date() {
    local date=""
    if git rev-parse --is-inside-work-tree &>/dev/null; then
        date=$(git log -1 --format=%cs -- "$1" 2>/dev/null || true)
    fi
    if [ -z "$date" ] && [ -f "$SOURCE_DIR/PAGE-DATES" ]; then
        date=$(awk -v page="$(basename "$1")" '$1 == page { print $2 }' \
            "$SOURCE_DIR/PAGE-DATES")
    fi
    if [ -z "$date" ]; then
        date=$(date -u -r "$1" +%Y-%m-%d)
    fi
    echo "$date"
}

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo -e "${CYAN}Rendering man pages for Zoi ${VERSION} from ${SOURCE_DIR}...${NC}"

# A malformed page must fail the release rather than ship a truncated man page,
# so a non-zero exit from asciidoctor propagates through `set -e` and aborts
# `archive.sh` before anything is signed or published.
#
# The manpage converter puts `mansource` and `docdate` in the footer, so the
# page's own source name (`:mansource:` in its header) gets the version
# appended, and `docdate` is set rather than left to the file's mtime.
for page in "$SOURCE_DIR"/*.adoc; do
    source=$(sed -n 's/^:mansource: *//p' "$page" | head -n 1)
    date=$(page_date "$page")
    asciidoctor -b manpage -D "$OUTPUT_DIR" \
        -a mansource="${source:-zoi} ${VERSION}" \
        -a docdate="$date" \
        -a revdate="$date" \
        "$page"
done

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
