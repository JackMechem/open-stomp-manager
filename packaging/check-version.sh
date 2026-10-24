#!/usr/bin/env bash
# The workspace Cargo.toml is the single source of truth for the version. Check that the README
# and CHANGELOG mention it, and (with an argument, e.g. from a release tag) that it matches.
#
#   packaging/check-version.sh            consistency check
#   packaging/check-version.sh 0.2.0      ... and the version must be 0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

v=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
ok=1
fail() { echo "error: $*" >&2; ok=0; }

[[ -n "$v" ]] || fail "no version in Cargo.toml"
[[ $# -eq 0 || "$1" == "$v" ]] || fail "expected version $1, Cargo.toml has $v"
# a pre-release (0.8.0-beta.1) leaves the README and CHANGELOG on the last stable release
if [[ "$v" != *-* ]]; then
    grep -q "Version \*\*$v\*\*" README.md || fail "README.md does not say 'Version **$v**'"
    grep -q "^## \[$v\]" CHANGELOG.md || fail "CHANGELOG.md has no '## [$v]' section"
fi
grep -q "name = \"open-stomp-manager\"" Cargo.lock && \
    awk '/name = "open-stomp-manager"/{getline; print}' Cargo.lock | grep -q "\"$v\"" || fail "Cargo.lock is stale (run cargo build)"

((ok)) && echo "version $v is consistent"
((ok))
