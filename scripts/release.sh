#!/usr/bin/env bash

set -euo pipefail

if [ -z "$CI_COMMIT_TAG" ]; then
    echo "This script should be run on a tag."
    exit 1
fi

if [ ! -d "scripts/archived" ]; then
    echo "Artifacts from package:archive job not found."
    exit 1
fi

VERSION=$(echo "$CI_COMMIT_TAG" | sed -r 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/')
echo "Updating files to version: ${VERSION}"

sed -i -e "s/^version = ".*"/version = "${VERSION}"/" "Cargo.toml"

CHECKSUMS_256="scripts/archived/checksums-256.txt"
CHECKSUMS_512="scripts/archived/checksums.txt"

extract_checksum() {
    grep "$1" "$2" | awk '{print $1}'
}

MAC_ARM_SHA256=$(extract_checksum 'zoi-macos-arm64.tar.zst' "$CHECKSUMS_256")
MAC_AMD_SHA256=$(extract_checksum 'zoi-macos-amd64.tar.zst' "$CHECKSUMS_256")
NIX_AMD_SHA256=$(extract_checksum 'zoi-linux-amd64.tar.zst' "$CHECKSUMS_256")
NIX_ARM_SHA256=$(extract_checksum 'zoi-linux-arm64.tar.zst' "$CHECKSUMS_256")
WIN_AMD_SHA256=$(extract_checksum 'zoi-windows-amd64.zip' "$CHECKSUMS_256")
AMD64_SHA512=$(extract_checksum 'zoi-linux-amd64.tar.zst' "$CHECKSUMS_512")
ARM64_SHA512=$(extract_checksum 'zoi-linux-arm64.tar.zst' "$CHECKSUMS_512")

SOURCE_ARCHIVE_NAME="Zoi-${CI_COMMIT_TAG}.tar.gz"
ARCHIVE_SHA512=$(extract_checksum "$SOURCE_ARCHIVE_NAME" "$CHECKSUMS_512")

LICENSE_SHA512=$(sha512sum LICENSE | awk '{print $1}')

sed -i -e "s/pkgver=ZOI_VERSION/pkgver=${VERSION}/g" \
       -e "s/AMD64_SHA512/${AMD64_SHA512}/g" \
       -e "s/ARM64_SHA512/${ARM64_SHA512}/g" \
       -e "s/LICENSE_SHA512/${LICENSE_SHA512}/g" \
       "packages/aur/zoi-bin/PKGBUILD"

sed -i -e "s/pkgver = ZOI_VERSION/pkgver = ${VERSION}/g" \
       -e "s/source_x86_64 = zoi-linux-amd64.tar.zst::.*$/source_x86_64 = zoi-linux-amd64.tar.zst::https:\/\/gitlab.com\/Zillowe\/Zillwen\/Zusty\/Zoi\/-\/releases\/${CI_COMMIT_TAG}\/downloads\/zoi-linux-amd64.tar.zst/g" \
       -e "s/sha512sums_x86_64 = AMD64_SHA512/sha512sums_x86_64 = ${AMD64_SHA512}/g" \
       -e "s/sha512sums_x86_64 = LICENSE_SHA512/sha512sums_x86_64 = ${LICENSE_SHA512}/g" \
       -e "s/source_aarch64 = zoi-linux-arm64.tar.zst::.*$/source_aarch64 = zoi-linux-arm64.tar.zst::https:\/\/gitlab.com\/Zillowe\/Zillwen\/Zusty\/Zoi\/-\/releases\/${CI_COMMIT_TAG}\/downloads\/zoi-linux-arm64.tar.zst/g" \
       -e "s/sha512sums_aarch64 = ARM64_SHA512/sha512sums_aarch64 = ${ARM64_SHA512}/g" \
       -e "s/sha512sums_aarch64 = LICESNE_SHA512/sha512sums_aarch64 = ${LICENSE_SHA512}/g" \
       "packages/aur/zoi-bin/.SRCINFO"

sed -i -e "s/pkgver=ZOI_VERSION/pkgver=${VERSION}/g" \
       -e "s/ARCHIVE_SHA512/${ARCHIVE_SHA512}/g" \
       -e "s/LICENSE_SHA512/${LICENSE_SHA512}/g" \
       -e "s#source=(\".*#source=(\"\$url/-/archive/${CI_COMMIT_TAG}/Zoi-${CI_COMMIT_TAG}.tar.gz\"#" \
       "packages/aur/zoi/PKGBUILD"

sed -i -e "s/pkgver = VERSION/pkgver = ${VERSION}/g" \
       -e "s/source = .*$/source = https:\/\/gitlab.com\/Zillowe\/Zillwen\/Zusty\/Zoi\/-\/archive\/${CI_COMMIT_TAG}\/Zoi-${CI_COMMIT_TAG}.tar.gz/g" \
       -e "s/sha512sums = ARCHIVE_SHA512/sha512sums = ${ARCHIVE_SHA512}/g" \
       -e "s/sha512sums = LICENSE_SHA512/sha512sums = ${LICENSE_SHA512}/g" \
       "packages/aur/zoi/.SRCINFO"

sed -i -e "s/version \"ZOI_VERSION\"/version \"${VERSION}\"" \
       -e "s#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-.*_VERSION}/downloads/zoi-macos-arm64.tar.zst\"#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/${CI_COMMIT_TAG}/downloads/zoi-macos-arm64.tar.zst\"#" \
       -e "s/sha256 \"MACOS_ARM64_SHA256\"/sha256 \"${MAC_ARM_SHA256}\"/" \
       -e "s#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-.*_VERSION}/downloads/zoi-macos-amd64.tar.zst\"#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/${CI_COMMIT_TAG}/downloads/zoi-macos-amd64.tar.zst\"#" \
       -e "s/sha256 \"MACOS_AMD64_SHA256\"/sha256 \"${MAC_AMD_SHA256}\"/" \
       -e "s#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-.*_VERSION}/downloads/zoi-linux-amd64.tar.zst\"#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/${CI_COMMIT_TAG}/downloads/zoi-linux-amd64.tar.zst\"#" \
       -e "s/sha256 \"LINUX_AMD64_SHA256\"/sha256 \"${NIX_AMD_SHA256}\"/" \
       -e "s#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/Prod-Release-.*_VERSION}/downloads/zoi-linux-arm64.tar.zst\"#url \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/${CI_COMMIT_TAG}/downloads/zoi-linux-arm64.tar.zst\"#" \
       -e "s/sha256 \"LINUX_ARM64_SHA256\"/sha256 \"${NIX_ARM_SHA256}\"/" \
       "packages/brew/zoi.rb"

sed -i -e "s/\"version\": \"ZOI_VERSION\"/\"version\": \"${VERSION}\"" \
       -e "s#\"url\": \".*\"#\"url\": \"https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/releases/${CI_COMMIT_TAG}/downloads/zoi-windows-amd64.zip\"#" \
       -e "s/\"hash\": \"AMD64_SHA256\"/\"hash\": \"${WIN_AMD_SHA256}\"" \
       -e "s#releases/Prod-Release-\\\$version/#releases/${CI_COMMIT_TAG}/#" \
       "packages/scoop/zoi.json"

echo "Package files updated successfully."

echo "--- Updating package manager files ---"
mkdir -p ~/.ssh
echo "$SSH_PRIVATE_KEY" | base64 -d > ~/.ssh/id_rsa
chmod 600 ~/.ssh/id_rsa
ssh-keyscan -H aur.archlinux.org >> ~/.ssh/known_hosts
ssh-keyscan -H github.com >> ~/.ssh/known_hosts
git config --global user.email "contact@zillowe.qzz.io"
git config --global user.name "Zillowe CI/CD"

echo "--- AUR ---"
git clone "ssh://aur@aur.archlinux.org/zoi-bin.git" aur_zoi_bin
cd aur_zoi_bin
cp ../packages/aur/zoi-bin/PKGBUILD .
cp ../packages/aur/zoi-bin/.SRCINFO .
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin master
else
  echo "No changes detected, skipping commit"
fi
cd ..

git clone "ssh://aur@aur.archlinux.org/zoi.git" aur_zoi
cd aur_zoi
cp ../packages/aur/zoi/PKGBUILD .
cp ../packages/aur/zoi/.SRCINFO .
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin master
else
  echo "No changes detected, skipping commit"
fi
cd ..

echo "--- Homebrew ---"
git clone "ssh://git@github.com/Zillowe/homebrew-tap" brew_zoi
cd brew_zoi
cp ../packages/brew/zoi.rb .
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin main
else
  echo "No changes detected, skipping commit"
fi
cd ..

echo "--- Scoop ---"
git clone "ssh://git@github.com/Zillowe/scoop.git" scoop_zoi
cd scoop_zoi
cd bucket
cp ../../packages/scoop/zoi.json .
if [[ -n $(git status --porcelain) ]]; then
  echo "Committing and pushing package updates..."
  git add .
  git commit -m "Release: Bump package version to ${VERSION}"
  git push origin main
else
  echo "No changes detected, skipping commit"
fi
cd ../..
