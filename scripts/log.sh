#!/bin/bash

set -e

if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <old_tag> <new_tag>"
    echo "Example: $0 v1.4.0 v1.5.0"
    exit 1
fi

OLD_TAG_NAME=$1
NEW_TAG_NAME=$2

OUTPUT_FILE="release_notes.md"

if [ -z "$OLD_TAG_NAME" ]; then
    echo "No previous tag provided, generating changelog from beginning..."
    git-cliff --tag "${NEW_TAG_NAME}" -o "${OUTPUT_FILE}"
else
    OLD_COMMIT=$(git rev-parse "${OLD_TAG_NAME}")
    NEW_COMMIT=$(git rev-parse "${NEW_TAG_NAME}")
    
    echo "Generating changelog from commit $OLD_COMMIT ($OLD_TAG_NAME) to $NEW_COMMIT ($NEW_TAG_NAME)..."
    git-cliff "${OLD_COMMIT}..${NEW_COMMIT}" -o "${OUTPUT_FILE}"
fi

echo "Changelog generated successfully: ${OUTPUT_FILE}"
