#!/bin/bash

set -e

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <old_tag> <new_tag>"
    echo "Example: $0 v1.4.0 v1.5.0"
    exit 1
fi

OLD_TAG=$1
NEW_TAG=$2
OUTPUT_FILE="release_notes.md"

echo "Generating changelog from $OLD_TAG to $NEW_TAG..."

git-cliff --tag-range "${OLD_TAG}..${NEW_TAG}" -o "${OUTPUT_FILE}"

echo "Changelog generated successfully: ${OUTPUT_FILE}"
