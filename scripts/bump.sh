#!/bin/bash
set -e

# Usage: ./scripts/bump.sh [Branch]-[Status]-[Number]
# Example: ./scripts/bump.sh Prod-Release-1.25.0
# Uses ZFVM (https://zillowe.qzz.io/docs/methods/zfvm)

VERSION_INPUT=$1

if [ -z "$VERSION_INPUT" ]; then
    echo "Usage: $0 [Branch]-[Status]-[Number]"
    echo "Example: $0 Prod-Release-1.25.0"
    exit 1
fi

# Expects format like: Prod-Release-1.25.0
B_SHORT=$(echo "$VERSION_INPUT" | cut -d'-' -f1)
STATUS=$(echo "$VERSION_INPUT" | cut -d'-' -f2)
NUMBER=$(echo "$VERSION_INPUT" | cut -d'-' -f3)

if [ -z "$B_SHORT" ] || [ -z "$STATUS" ] || [ -z "$NUMBER" ]; then
    echo "Error: Invalid version format. Expected [Branch]-[Status]-[Number]"
    exit 1
fi

# Map Branch Names and Suffixes
case $B_SHORT in
Prod)
    BRANCH="Production"
    SUFFIX=""
    ;;
Dev)
    BRANCH="Development"
    SUFFIX="-dev"
    ;;
Pub)
    BRANCH="Public"
    SUFFIX="-pub"
    ;;
Spec)
    BRANCH="Special"
    SUFFIX="-spec"
    ;;
*)
    echo "Error: Unknown branch prefix '$B_SHORT'. Expected Prod, Dev, Pub, or Spec."
    exit 1
    ;;
esac

CARGO_VERSION="${NUMBER}${SUFFIX}"

echo ":: Bumping to $VERSION_INPUT (Cargo: $CARGO_VERSION)"

# - Update crates/cli/src/cli.rs
echo ":: Updating crates/cli/src/cli.rs..."
sed -i "s/const BRANCH: \&str = \".*\";/const BRANCH: \&str = \"$BRANCH\";/" crates/cli/src/cli.rs
sed -i "s/const STATUS: \&str = \".*\";/const STATUS: \&str = \"$STATUS\";/" crates/cli/src/cli.rs
sed -i "s/const NUMBER: \&str = \".*\";/const NUMBER: \&str = \"$NUMBER\";/" crates/cli/src/cli.rs

# - Update Cargo.toml
# We fetch the current workspace version to perform a surgical global replacement
# of all internal version strings.
CURRENT_CARGO_VER=$(grep -m 1 "version =" Cargo.toml | tr -d ' ' | cut -d'"' -f2)

if [ -n "$CURRENT_CARGO_VER" ]; then
    echo ":: Replacing all occurrences of version \"$CURRENT_CARGO_VER\" with \"$CARGO_VERSION\" in Cargo.toml..."
    sed -i "s/version = \"$CURRENT_CARGO_VER\"/version = \"$CARGO_VERSION\"/g" Cargo.toml
else
    echo "Warning: Could not detect current Cargo version. Skipping bulk replace."
fi

# - Update Cargo.lock
echo ":: Running cargo check to update Cargo.lock..."
cargo check --workspace --quiet

echo ":: Successfully bumped to $VERSION_INPUT!"
