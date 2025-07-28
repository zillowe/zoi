#!/bin/sh
set -e

OS_TYPE=$(uname -s)
ARCH=${1:-amd64} 

RELEASE_DIR="./build/release"
mkdir -p "$RELEASE_DIR"

if [ "$ARCH" = "amd64" ]; then
    RUST_ARCH="x86_64"
elif [ "$ARCH" = "arm64" ]; then
    RUST_ARCH="aarch64"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

if [ "$OS_TYPE" = "FreeBSD" ]; then
    TARGET="$RUST_ARCH-unknown-freebsd"
    OUTPUT_NAME="zoi-freebsd-$ARCH"
elif [ "$OS_TYPE" = "OpenBSD" ]; then
    TARGET="$RUST_ARCH-unknown-openbsd"
    OUTPUT_NAME="zoi-openbsd-$ARCH"
else
    echo "Unsupported OS for this script: $OS_TYPE"
    exit 1
fi

if [ "$OS_TYPE" = "OpenBSD" ] && [ "$ARCH" = "arm64" ]; then
    echo "Skipping OpenBSD/arm64 build as it is not currently supported."
    exit 0
fi

echo "--- Building for $TARGET ---"
cargo build --release --target "$TARGET"
cp "./target/$TARGET/release/zoi" "$RELEASE_DIR/$OUTPUT_NAME"

echo "--- Build complete for $OS_TYPE ($ARCH) ---"
ls -l "$RELEASE_DIR"
