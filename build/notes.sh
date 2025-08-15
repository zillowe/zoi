#!/bin/bash

# This script generates release notes.
# It first tries to use Gemini, and falls back to a legacy method on failure.

set -e

generate_legacy_notes() {
    echo "Falling back to legacy release note generation." >&2
    if [ $(git tag -l | wc -l) -gt 1 ]; then
        PREVIOUS_TAG=$(git describe --tags --abbrev=0 $(git rev-list --tags --skip=1 --max-count=1))
        COMMIT_RANGE="$PREVIOUS_TAG..$CI_COMMIT_TAG"
    else
        COMMIT_RANGE="$CI_COMMIT_TAG"
    fi
    
    CHANGELOG=$(git log --pretty=format:'- %s ([%h]('"$CI_PROJECT_URL"'/-/commit/%H))' "$COMMIT_RANGE")
    MERGED_MRS=$(git log "$COMMIT_RANGE" | grep -oE 'See merge request !([0-9]+)' | sed 's/See merge request/!/' | sort -u)
    CLOSED_ISSUES=$(git log "$COMMIT_RANGE" | grep -oE '(Closes|closes|Fixes|fixes) #[0-9]+' | sed -E 's/.*#/#/' | sort -u)
    
    echo "## Changelog"
    echo ""
    echo "$CHANGELOG"
    
    if [ -n "$MERGED_MRS" ]; then
        MR_LIST=$(echo "$MERGED_MRS" | sed 's/^/* /')
        echo ""
        echo "### Merged Merge Requests"
        echo "$MR_LIST"
    fi
    
    if [ -n "$CLOSED_ISSUES" ]; then
        ISSUE_LIST=$(echo "$CLOSED_ISSUES" | sed 's/^/* /')
        echo ""
        echo "### Closed Issues"
        echo "$ISSUE_LIST"
    fi
}

generate_gemini_notes() {
    echo "Attempting to generate release notes with Gemini." >&2
    if [ -z "$GEMINI_API_KEY_B64" ]; then
        echo "GEMINI_API_KEY_B64 not set. Cannot use Gemini." >&2
        return 1
    fi

    echo "GEMINI_API_KEY=$(echo "$GEMINI_API_KEY_B64" | base64 -d)" > .env
    source .env

    if [ -z "$GEMINI_API_KEY" ]; then
        echo "Failed to decode or source GEMINI_API_KEY." >&2
        return 1
    fi

    if [ $(git tag -l | wc -l) -gt 1 ]; then
        PREVIOUS_TAG=$(git describe --tags --abbrev=0 $(git rev-list --tags --skip=1 --max-count=1))
        COMMIT_RANGE="$PREVIOUS_TAG..$CI_COMMIT_TAG"
    else
        COMMIT_RANGE="$CI_COMMIT_TAG"
    fi

    COMMIT_LOG=$(git log --pretty=format:"### %s%n%n%b" "$COMMIT_RANGE")
    MERGED_MRS=$(git log $COMMIT_RANGE | grep -oE 'See merge request !([0-9]+)' | sed 's/See merge request/!/' | sort -u | paste -sd ' ' -)
    CLOSED_ISSUES=$(git log $COMMIT_RANGE | grep -oE '(Closes|closes|Fixes|fixes) #[0-9]+' | sed -E 's/.*#/#/' | sort -u | paste -sd ' ' -)

    PROMPT="Generate release notes in Markdown for version '$CI_COMMIT_TAG_MESSAGE'. The full commit message is provided below, use it as the primary source for the release notes. Also, summarize the following commits, merged MRs, and closed issues. Organize the summary into '### ✨ Features' and '### 🌟 Enhancements' and '### 🐛 Bug Fixes' . Be concise and professional.


**Commits:**
$COMMIT_LOG

**Merged MRs:** $MERGED_MRS

**Closed Issues:** $CLOSED_ISSUES
"
    timeout 420 gemini --prompt "$PROMPT"
}


generate_gemini_notes || generate_legacy_notes
