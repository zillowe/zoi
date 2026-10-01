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
# ZFVM Symbolic Form B is Branch-Status-Number, but `Pre-Alpha` contains a
# hyphen, so a positional `cut` of `Dev-Pre-Alpha-0.1.0` yields status `Pre`
# and number `Alpha`, silently writing garbage into cli.rs. Match against the
# fixed vocabularies instead, longest status first.
B_SHORT=""
STATUS=""
NUMBER=""
REST="$VERSION_INPUT"

for b in Prod Dev Spec Pub; do
    if [[ "$REST" == "$b-"* ]]; then
        B_SHORT="$b"
        REST="${REST#"$b-"}"
        break
    fi
done

for s in Pre-Alpha Release Alpha Beta RC; do
    if [[ "$REST" == "$s-"* ]]; then
        STATUS="$s"
        REST="${REST#"$s-"}"
        break
    fi
done
NUMBER="$REST"

if [ -z "$B_SHORT" ] || [ -z "$STATUS" ] || [ -z "$NUMBER" ]; then
    echo "Error: Invalid version format. Expected [Branch]-[Status]-[Number]"
    echo "  Valid branches: Prod, Dev, Spec, Pub"
    echo "  Valid statuses: Pre-Alpha, Alpha, Beta, RC, Release"
    exit 1
fi

# - A full SemVer check, including the no-leading-zero rule that ZFVM inherits.
# - Note this must use `=~`, not `[ == ]`: `test` compares the right-hand side
# - as a literal string rather than a glob, so `[ "$NUMBER" == *.*.* ]` rejects
# - every valid version.
if [[ ! "$NUMBER" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    echo "Error: '$NUMBER' is not a valid Major.Minor.Patch version."
    echo "  Expected non-negative integers without leading zeros, e.g. 1.2.3"
    exit 1
fi

# Map the branch to its ZFVM identifier.
# BRANCH holds the identifier (Prod, Dev, Spec, Pub), not the long name: the
# ZFVM Canonical Form requires the identifier, so `Development Beta 1.2.3` is
# not a valid version string while `Dev Beta 1.2.3` is. Display code does the
# long-form mapping.
case $B_SHORT in
Prod)
    BRANCH="Prod"
    BRANCH_SLUG="prod"
    ;;
Dev)
    BRANCH="Dev"
    BRANCH_SLUG="dev"
    ;;
Pub)
    BRANCH="Pub"
    BRANCH_SLUG="pub"
    ;;
Spec)
    BRANCH="Spec"
    BRANCH_SLUG="spec"
    ;;
*)
    echo "Error: Unknown branch prefix '$B_SHORT'. Expected Prod, Dev, Pub, or Spec."
    exit 1
    ;;
esac

# Project the ZFVM version onto the Cargo version.
#
# Cargo has one pre-release slot and SemVer uses it for a single ordering
# axis, while ZFVM has two orthogonal axes: branch and status. Both have to
# survive into the Cargo version, because crates.io rejects a duplicate
# version and a build that reuses one is indistinguishable from an earlier
# one.
#
# `Prod Release X.Y.Z` is the only combination that projects to a bare
# `X.Y.Z`, and it is the only combination `.gitlab-ci.yml` publishes to
# crates.io. Every other combination takes the pre-release slot as
# `X.Y.Z-<branch>.<status-ordinal>`, for example:
#
#   Prod Beta 1.29.0      -> 1.29.0-prod.2
#   Dev   Release 1.29.0  -> 1.29.0-dev.4
#   Pub   RC     2.0.0    -> 2.0.0-pub.3
#
# The ordinal is numeric rather than a readable label on purpose: SemVer
# compares numeric identifiers before lexical ones, so `-pre-alpha` would
# otherwise sort after `-beta`. Both components are separated by a dot, and
# both sit in the pre-release slot, so every one of these sorts below the
# corresponding bare `X.Y.Z`.
if [ "$BRANCH" = "Prod" ] && [ "$STATUS" = "Release" ]; then
    CARGO_VERSION="$NUMBER"
else
    if command -v "${ZFVM:-zfvm}" >/dev/null 2>&1; then
        # Read the ordinal from the `zfvm` crate rather than duplicating the
        # mapping here, so the three places that must agree cannot drift.
        ORDINAL=$("${ZFVM:-zfvm}" statuses | awk -v s="$STATUS" \
            '$4 == s { print $2 }')
    else
        # Fallback table, mirroring ZFVM section 10. Used only when the crate
        # is unavailable; the `zfvm parse` check below warns about that.
        case $STATUS in
        Pre-Alpha) ORDINAL=0 ;;
        Alpha) ORDINAL=1 ;;
        Beta) ORDINAL=2 ;;
        RC) ORDINAL=3 ;;
        Release) ORDINAL=4 ;;
        esac
    fi

    if [ -z "$ORDINAL" ]; then
        echo "Error: Could not determine the ZFVM ordinal for status '$STATUS'."
        exit 1
    fi

    CARGO_VERSION="${NUMBER}-${BRANCH_SLUG}.${ORDINAL}"
fi

# - Cross-check against the `zfvm` crate when it is available. `cargo install
# - zfvm-cli`. A version the specification rejects should never reach Cargo or
# - a release tag, and this catches policy violations such as a `Release`
# - status on a zero major version, which the shell validation above accepts
# - because the string is well-formed.
if command -v "${ZFVM:-zfvm}" >/dev/null 2>&1; then
    CANONICAL="${BRANCH} ${STATUS} ${NUMBER}"
    if ! "${ZFVM:-zfvm}" parse "$CANONICAL" >/dev/null 2>&1; then
        echo "Error: '$CANONICAL' is not a valid ZFVM version."
        echo "  See ${ZFVM:-zfvm} parse '$CANONICAL' for details."
        exit 1
    fi
    # - Surface a policy warning without blocking. `parse` reports the
    # - Release-with-zero-major rule as a warning, not an error.
    POLICY=$("${ZFVM:-zfvm}" parse "$CANONICAL" 2>/dev/null | grep '^Warning' || true)
    if [ -n "$POLICY" ]; then
        echo ":: Warning:${POLICY#Warning}"
        echo "::   ZFVM 2.1 section 6.1: a 0.y.z codebase has no stable public API,"
        echo "::   so it cannot carry the Release status. Use RC or below."
    fi
else
    echo ":: Note: zfvm not found in PATH, using the built-in ZFVM tables."
    echo ":: Install it with 'cargo install zfvm-cli' so the ordinals come"
    echo ":: from the crate rather than a copy in this script."
fi

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
# - `cargo check` is skipped when ZOI_BUMP_SKIP_LOCK is set, which is what the
# - - test harness uses to sweep many versions quickly. The lock file is left
# - - stale in that case and must be regenerated before building.
if [ "${ZOI_BUMP_SKIP_LOCK:-}" = "1" ]; then
    echo ":: Skipping Cargo.lock update (ZOI_BUMP_SKIP_LOCK=1)."
else
    echo ":: Running cargo check to update Cargo.lock..."
    cargo check --workspace --quiet
fi

echo ":: Successfully bumped to $VERSION_INPUT!"
