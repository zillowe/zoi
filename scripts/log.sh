#!/bin/bash

set -e

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <old_tag> <new_tag>"
    echo "Example: $0 v1.4.0 v1.5.0"
    exit 1
fi

OLD_TAG_NAME=$1
NEW_TAG_NAME=$2

OLD_COMMIT=$(git rev-parse "${OLD_TAG_NAME}")
NEW_COMMIT=$(git rev-parse "${NEW_TAG_NAME}")

OUTPUT_FILE="release_notes.md"

echo "Generating changelog from commit $OLD_COMMIT ($OLD_TAG_NAME) to $NEW_COMMIT ($NEW_TAG_NAME)..."

git-cliff "${OLD_COMMIT}..${NEW_COMMIT}" -o "${OUTPUT_FILE}"

echo "Changelog generated successfully: ${OUTPUT_FILE}"
