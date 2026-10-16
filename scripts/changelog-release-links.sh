#!/usr/bin/env bash
#
# Rewrites the version headings of a git-cliff changelog so that each one
# points at its GitLab release page, falling back to the tag page for the tags
# that have no release.
#
# Why a postprocessor: git-cliff templates are rendered from the git history
# alone and cannot know whether a release exists on the remote, while
# `/-/releases/<tag>` is a 404 and `/-/tags/<tag>` is not. The body template in
# cliff.toml therefore always links the tag page, and this script upgrades the
# headings of the tags that do have a release.
#
# git-cliff runs this once per rendered release, with that release on stdin and
# its own stdout in place of it, so the release list is cached: without the cache
# a single `git cliff` would ask GitLab for the release list once per release.
#
# Usage:
#   scripts/changelog-release-links.sh < CHANGELOG.md > CHANGELOG.md.new
#   scripts/changelog-release-links.sh CHANGELOG.md > CHANGELOG.md.new
#
# Override the remote with ZOI_GITLAB_PROJECT (`group/subgroup/project`) and
# ZOI_GITLAB_URL (`https://gitlab.com/group/subgroup/project`) when vendoring.
# ZOI_CHANGELOG_RELEASE_TTL is how long, in seconds, the cached release list is
# reused; 0 forces a lookup on every invocation.

set -o pipefail

PROJECT="${ZOI_GITLAB_PROJECT:-zillowe/zillwen/zusty/zoi}"
PROJECT_URL="${ZOI_GITLAB_URL:-https://gitlab.com/zillowe/zillwen/zusty/zoi}"
TTL="${ZOI_CHANGELOG_RELEASE_TTL:-600}"
# `glab` only resolves a URL-encoded project path here: with raw slashes it
# walks the `projects` endpoint as a filesystem path and answers 404.
PROJECT_ID="${PROJECT//\//%2F}"
CACHE="${XDG_CACHE_HOME:-${HOME:-/tmp}/.cache}/zoi/releases-${PROJECT//\//-}.txt"

if [ "$#" -gt 0 ]; then
    CHANGELOG="$1"
else
    # git-cliff pipes the changelog in, and the heredoc below feeds the Python
    # program from stdin, so the changelog has to be spooled to a file first
    # rather than read back out of /dev/stdin.
    CHANGELOG="$(mktemp)"
    trap 'rm -f "$CHANGELOG"' EXIT
    cat >"$CHANGELOG"
fi

# Ask GitLab which tags carry a release. `glab` is the only client used here and
# it carries the token from its own config, so no extra credentials are needed.
# A failure is not fatal: an empty list only means every heading keeps its tag
# link, which is still a working URL.
fetch_release_tags() {
    if ! command -v glab >/dev/null 2>&1; then
        echo ":: Note: glab not found in PATH, linking every version to its tag." >&2
        return 0
    fi
    # `glab api` prints a whole page on one line, so extract every occurrence on
    # its own line instead of matching once per line.
    glab api "projects/$PROJECT_ID/releases?per_page=100" --paginate 2>/dev/null |
        grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' |
        cut -d '"' -f4
}

release_tags() {
    local age fetched
    if [ -f "$CACHE" ]; then
        # `date -r` reads an mtime on both GNU and BSD, unlike `stat`.
        age=$(($(date +%s) - $(date -r "$CACHE" +%s)))
        if [ "$age" -lt "$TTL" ]; then
            cat "$CACHE"
            return 0
        fi
    fi

    fetched="$(fetch_release_tags)"
    if [ -z "$fetched" ] && [ -f "$CACHE" ]; then
        # An API hiccup should not forget every release and downgrade the whole
        # changelog to tag links, so keep the previous list.
        fetched="$(cat "$CACHE")"
    fi
    mkdir -p "$(dirname "$CACHE")"
    printf '%s\n' "$fetched" >"$CACHE"
    printf '%s\n' "$fetched"
}

RELEASES="$(release_tags)"

# The heading rewrite is easier to read in Python than in awk.
python3 - "$RELEASES" "$CHANGELOG" "$PROJECT_URL" <<'PYTHON'
import re
import subprocess
import sys

releases, path, project_url = sys.argv[1], sys.argv[2], sys.argv[3]

# Comparing whole strings, never prefixes, so that `Prod-Release-1.2` cannot be
# mistaken for `Prod-Release-1.2.3`.
has_release = set(releases.splitlines())

# A heading git-cliff already linked, as rendered from the cliff.toml body:
#   ## [Prod. Release 1.28.0](https://gitlab.com/o/r/-/tags/Prod-Release-1.28.0)
linked = re.compile(r"^(## \[)([^]]+)(\]\()(.*/-/)(?:tags|releases)/([^)]*)(\))(.*)$")

# A heading without any URL, which is what the static cliff.toml footer holds.
bare = re.compile(r"^(## \[)([^]]+)(\])(.*)$")


def tag_from_label(label):
    """Map a rendered heading back onto its tag, the inverse of the
    `replace_regex` chain in the cliff.toml body.

    `Prod. Release 1.28.0` and `Dev. Pre-Alpha 2.4.0` become
    `Prod-Release-1.28.0` and `Dev-Pre-Alpha-2.4.0`. A label of another shape,
    such as `unreleased`, returns `None`.
    """
    match = re.fullmatch(
        r"([A-Za-z]+)\. ([A-Za-z]+(?:-[A-Za-z]+)*) ([0-9]+(?:\.[0-9]+)*)", label
    )
    if not match:
        return None
    return "-".join(match.groups())


def page_for(tag):
    """Release page for a tag that has a release, tag page otherwise."""
    kind = "releases" if tag in has_release else "tags"
    return f"{project_url}/-/{kind}/{tag}"


def tag_exists(tag):
    return subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0


def rewrite(line):
    """Rewrite one heading. The line arrives without its line ending, because a
    regex anchored with `$` would otherwise swallow it."""
    match = linked.match(line)
    if match:
        prefix, label, open_url, base, tag, close_url, rest = match.groups()
        kind = "releases" if tag in has_release else "tags"
        return f"{prefix}{label}{open_url}{base}{kind}/{tag}{close_url}{rest}"

    match = bare.match(line)
    if match:
        prefix, label, close_bracket, rest = match.groups()
        tag = tag_from_label(label)
        # Only link a footer heading when the repository really carries that
        # tag, since a link to a nonexistent tag is worse than no link.
        if tag is None or not tag_exists(tag):
            return line
        return f"{prefix}{label}{close_bracket}({page_for(tag)}){rest}"

    return line


with open(path) as handle:
    for line in handle:
        ending = "\n" if line.endswith("\n") else ""
        body = line[: -len(ending)] if ending else line
        sys.stdout.write(rewrite(body) + ending)
PYTHON
