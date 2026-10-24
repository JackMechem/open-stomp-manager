#!/usr/bin/env bash
# Point the `stable` and `unstable` branches at the newest published GitHub releases, so Nix (or a
# cargo git dependency) can follow a channel instead of a version tag:
#
#   stable     the newest release that is not a pre-release (vx.y.z)
#   unstable   the newest release of any kind (vx.y.z-beta.n or vx.y.z), so never behind stable
#
# Drafts and tags without a release are skipped. .github/workflows/channels.yml runs this after
# each release; it needs gh (GH_TOKEN) and the tags fetched.
#
#   packaging/update-channels.sh         move the branches
#   packaging/update-channels.sh -n      only print where they would go
set -euo pipefail
cd "$(dirname "$0")/.."

dry=0
[[ "${1:-}" == "-n" ]] && dry=1

# "<tag> pre" for a pre-release, "<tag> stable" otherwise
releases=$(gh release list --limit 1000 --exclude-drafts --json tagName,isPrerelease \
    --jq '.[] | .tagName + (if .isPrerelease then " pre" else " stable" end)')

stable= unstable=
# newest first; versionsort.suffix puts v0.6.0-beta.1 below v0.6.0
for t in $(git -c versionsort.suffix=- tag -l 'v*' --sort=-version:refname); do
    kind=$(awk -v t="$t" '$1 == t { print $2 }' <<<"$releases")
    [[ -n "$kind" ]] || continue
    [[ -n "$unstable" ]] || unstable=$t
    if [[ "$kind" == stable ]]; then
        stable=$t
        break
    fi
done

move() { # <branch> <tag>
    local sha
    sha=$(git rev-parse "$2^{commit}")
    echo "$1 -> $2 ($sha)"
    ((dry)) || git push --force origin "$sha:refs/heads/$1"
}
[[ -z "$stable" ]] || move stable "$stable"
[[ -z "$unstable" ]] || move unstable "$unstable"
