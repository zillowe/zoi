#!/usr/bin/env bash

set -euo pipefail

if [ -z "${CI_COMMIT_TAG:-}" ]; then
  echo "Error: CI_COMMIT_TAG is not set"
  exit 1
fi

VERSION="${CI_COMMIT_TAG#Prod-Release-}"
echo "Publishing version: $VERSION"

ARCHIVE_DIR="./scripts/archived"
CHECKSUM_FILE="${ARCHIVE_DIR}/checksums.txt"
CHECKSUM_SHA256_FILE="${ARCHIVE_DIR}/checksums-256.txt"

# Looks up a digest by exact asset name.
#
# The match is anchored rather than a substring grep because the manifest holds
# every release asset, and an unanchored search for a short name such as
# "LICENSE" or "zoi.1" would happily return the hash of a different file that
# merely contains that text. Entries are written as `./<name>` by `sha512sum`
# over `find .`, except the source archive, which `archive.sh` appends with no
# prefix, so both shapes are accepted.
function lookup_digest() {
  local checksum_file=$1
  local asset=$2

  awk -v asset="$asset" '
        {
            entry = $2
            sub(/^\.\//, "", entry)
            if (entry == asset) {
                print $1
                found = 1
                exit
            }
        }
        END { exit(found ? 0 : 1) }
    ' "$checksum_file"
}

function require_digest() {
  local digest
  digest=$(lookup_digest "$1" "$2") || {
    echo "Error: no digest for '$2' in $1"
    echo "The release did not publish it. Check that 'scripts/man.sh' ran and that 'scripts/archive.sh' picked the asset up."
    exit 1
  }
  echo "$digest"
}

function get_sha512() {
  require_digest "$CHECKSUM_FILE" "$1"
}
function get_sha256() {
  require_digest "$CHECKSUM_SHA256_FILE" "$1"
}

SHA512_SRC=$(get_sha512 "Zoi-${CI_COMMIT_TAG}.tar.gz")
SHA512_LINUX_AMD64=$(get_sha512 "zoi-linux-amd64.tar.zst")
SHA512_LINUX_ARM64=$(get_sha512 "zoi-linux-arm64.tar.zst")

SHA256_MACOS_ARM64=$(get_sha256 "zoi-macos-arm64.tar.zst")
SHA256_MACOS_AMD64=$(get_sha256 "zoi-macos-amd64.tar.zst")
SHA256_LINUX_AMD64=$(get_sha256 "zoi-linux-amd64.tar.zst")
SHA256_LINUX_ARM64=$(get_sha256 "zoi-linux-arm64.tar.zst")
SHA256_WINDOWS_AMD64=$(get_sha256 "zoi-windows-amd64.zip")

# The man pages and the LICENSE are published as signed release assets by
# `scripts/man.sh` and `scripts/archive.sh`, so downstream packages fetch them
# from the release instead of rendering the AsciiDoc at build time. That keeps
# asciidoctor, and with it a Ruby toolchain, out of every package's build
# dependency closure.
SHA512_LICENSE=$(get_sha512 "LICENSE")
SHA512_MAN_ZOI=$(get_sha512 "zoi.1")
SHA512_MAN_ZOI_RS=$(get_sha512 "zoi-rs.3")
SHA512_MAN_ZOI_LUA=$(get_sha512 "zoi-lua.5")

SHA256_MAN_ZOI=$(get_sha256 "zoi.1")
SHA256_MAN_ZOI_RS=$(get_sha256 "zoi-rs.3")
SHA256_MAN_ZOI_LUA=$(get_sha256 "zoi-lua.5")

TMP_PACKAGES=$(mktemp -d)
chmod 755 "$TMP_PACKAGES"
cp -r packages/* "$TMP_PACKAGES/"

find "$TMP_PACKAGES" -type f -exec sed -i \
  -e "s/__VERSION__/${VERSION}/g" \
  -e "s/__SHA512_SRC__/${SHA512_SRC}/g" \
  -e "s/__SHA512_LINUX_AMD64__/${SHA512_LINUX_AMD64}/g" \
  -e "s/__SHA512_LINUX_ARM64__/${SHA512_LINUX_ARM64}/g" \
  -e "s/__SHA256_MACOS_ARM64__/${SHA256_MACOS_ARM64}/g" \
  -e "s/__SHA256_MACOS_AMD64__/${SHA256_MACOS_AMD64}/g" \
  -e "s/__SHA256_LINUX_AMD64__/${SHA256_LINUX_AMD64}/g" \
  -e "s/__SHA256_LINUX_ARM64__/${SHA256_LINUX_ARM64}/g" \
  -e "s/__SHA256_WINDOWS_AMD64__/${SHA256_WINDOWS_AMD64}/g" \
  -e "s/__SHA512_LICENSE__/${SHA512_LICENSE}/g" \
  -e "s/__SHA512_MAN_ZOI__/${SHA512_MAN_ZOI}/g" \
  -e "s/__SHA512_MAN_ZOI_RS__/${SHA512_MAN_ZOI_RS}/g" \
  -e "s/__SHA512_MAN_ZOI_LUA__/${SHA512_MAN_ZOI_LUA}/g" \
  -e "s/__SHA256_MAN_ZOI__/${SHA256_MAN_ZOI}/g" \
  -e "s/__SHA256_MAN_ZOI_RS__/${SHA256_MAN_ZOI_RS}/g" \
  -e "s/__SHA256_MAN_ZOI_LUA__/${SHA256_MAN_ZOI_LUA}/g" \
  {} +

echo "Generating .SRCINFO for AUR packages..."
chown -R nobody "$TMP_PACKAGES/aur"
cd "$TMP_PACKAGES/aur/zoi"
sudo -u nobody makepkg --printsrcinfo >.SRCINFO
cd ../zoi-bin
sudo -u nobody makepkg --printsrcinfo >.SRCINFO
cd ../../../

echo "--- Updating package manager files ---"
mkdir -p ~/.ssh
echo "$SSH_PRIVATE_KEY" | base64 -d >~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa
ssh-keyscan -H aur.archlinux.org >>~/.ssh/known_hosts
ssh-keyscan -H github.com >>~/.ssh/known_hosts
git config --global user.email "contact@zillowe.qzz.io"
git config --global user.name "Zillowe CI/CD"

echo "--- AUR ---"
git clone "ssh://aur@aur.archlinux.org/zoi-bin.git" aur_zoi_bin
cp "$TMP_PACKAGES/aur/zoi-bin/PKGBUILD" aur_zoi_bin/
cp "$TMP_PACKAGES/aur/zoi-bin/.SRCINFO" aur_zoi_bin/
cd aur_zoi_bin
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin master
fi
cd ..

git clone "ssh://aur@aur.archlinux.org/zoi.git" aur_zoi
cp "$TMP_PACKAGES/aur/zoi/PKGBUILD" aur_zoi/
cp "$TMP_PACKAGES/aur/zoi/.SRCINFO" aur_zoi/
cd aur_zoi
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin master
fi
cd ..

echo "--- Homebrew ---"
git clone "ssh://git@github.com/zillowe/homebrew-tap" brew_zoi
cp "$TMP_PACKAGES/brew/zoi.rb" brew_zoi/
cd brew_zoi
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin main
fi
cd ..

echo "--- Scoop ---"
git clone "ssh://git@github.com/zillowe/scoop.git" scoop_zoi
cp "$TMP_PACKAGES/scoop/zoi.json" scoop_zoi/bucket/
cd scoop_zoi
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin main
fi
cd ..

echo "--- Fedora COPR ---"
git clone "https://oauth2:${GITLAB_TOKEK_COPR}@gitlab.com/zillowe/packaging/copr.git" copr
mkdir -p copr/zoi
cp "$TMP_PACKAGES/rpm/zoi.spec" copr/zoi/
cd copr/
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release(Zoi): $VERSION"
  git push "https://oauth2:${GITLAB_TOKEN_COPR}@gitlab.com/zillowe/packaging/copr.git" HEAD:main
fi
cd ..
