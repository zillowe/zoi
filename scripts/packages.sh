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

function get_sha512() {
  grep "$1" "$CHECKSUM_FILE" | awk '{print $1}'
}
function get_sha256() {
  grep "$1" "$CHECKSUM_SHA256_FILE" | awk '{print $1}'
}

SHA512_SRC=$(get_sha512 "Zoi-${CI_COMMIT_TAG}.tar.gz")
SHA512_LINUX_AMD64=$(get_sha512 "zoi-linux-amd64.tar.zst")
SHA512_LINUX_ARM64=$(get_sha512 "zoi-linux-arm64.tar.zst")

SHA256_MACOS_ARM64=$(get_sha256 "zoi-macos-arm64.tar.zst")
SHA256_MACOS_AMD64=$(get_sha256 "zoi-macos-amd64.tar.zst")
SHA256_LINUX_AMD64=$(get_sha256 "zoi-linux-amd64.tar.zst")
SHA256_LINUX_ARM64=$(get_sha256 "zoi-linux-arm64.tar.zst")
SHA256_WINDOWS_AMD64=$(get_sha256 "zoi-windows-amd64.zip")

TMP_PACKAGES=$(mktemp -d)
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
git clone "ssh://git@github.com/Zillowe/homebrew-tap" brew_zoi
cp "$TMP_PACKAGES/brew/zoi.rb" brew_zoi/
cd brew_zoi
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin main
fi
cd ..

echo "--- Scoop ---"
git clone "ssh://git@github.com/Zillowe/scoop.git" scoop_zoi
cp "$TMP_PACKAGES/scoop/zoi.json" scoop_zoi/bucket/
cd scoop_zoi
if [[ -n $(git status --porcelain) ]]; then
  git add .
  git commit -m "Release: $VERSION"
  git push origin main
fi
cd ..
