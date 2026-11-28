#!/usr/bin/env bash

# Creates the release's official source tarball, `zoi-<version>.tar.gz`.
#
# It is `git archive` of the tagged commit plus two generated files that a
# build from the tarball would otherwise lose with `.git`:
#
# - `COMMIT`: the short commit hash, which `just build` embeds in the binary.
# - `man/PAGE-DATES`: each man page's last-change date, which `just man`
#   prints in the page footer.
#
# In the release it lands in `scripts/release/source/` (a subdirectory, so
# the binary loop in `archive.sh` does not take it for a binary) and is
# picked up by `archive.sh`,
# which signs it and lists it in `checksums.txt`. Package definitions build
# from this tarball rather than from GitLab's generated tag archive.
#
# Usage: scripts/source-tarball.sh [OUTPUT_DIR]   (default: scripts/release/source)

set -euo pipefail

# Globs below (man/*.adoc) sort by the locale's collation; the C locale
# keeps the generated files identical on every machine.
export LC_ALL=C

OUTPUT_DIR="${1:-./scripts/release/source}"

VERSION=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n 1)
if [ -z "$VERSION" ]; then
    echo "Error: could not read the version from Cargo.toml." >&2
    exit 1
fi

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "Warning: uncommitted changes are not included; the tarball is" \
        "made from the last commit ($(git rev-parse --short=10 HEAD))." >&2
fi

PREFIX="zoi-${VERSION}"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

git archive --format=tar --prefix="${PREFIX}/" HEAD | tar -x -C "$WORK"

git rev-parse --short=10 HEAD >"$WORK/$PREFIX/COMMIT"
for page in man/*.adoc; do
    printf '%s %s\n' "$(basename "$page")" "$(git log -1 --format=%cs -- "$page")"
done >"$WORK/$PREFIX/man/PAGE-DATES"

# Reproducible: sorted entries, fixed owner, every mtime set to the commit's
# time, and fixed permissions (0644, or 0755 for directories and executables).
# The permissions must be set here: `git archive` stores 0664/0775 (git's
# `tar.umask` is 0002), which root's `tar -x` keeps but a normal user's umask
# changes, so the same commit used to give a different tarball in CI (root)
# than on a developer machine.
EPOCH=$(git log -1 --format=%ct HEAD)
mkdir -p "$OUTPUT_DIR"
tar --sort=name --owner=0 --group=0 --numeric-owner \
    --mode='u+rwX,go+rX,go-w' \
    --mtime="@${EPOCH}" -C "$WORK" -cf - "$PREFIX" |
    gzip -n -9 >"$OUTPUT_DIR/${PREFIX}.tar.gz"

echo "Created $OUTPUT_DIR/${PREFIX}.tar.gz"
