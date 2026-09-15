#!/usr/bin/env bash
# Sets the version everywhere it is written down.
#
#   scripts/set-version.sh 0.2.0
#
# package.json is the source of truth: tauri.conf.json reads it, so the desktop bundle, the
# window's About box and the installer all follow from one edit. The Cargo workspace carries
# its own copy, which every crate inherits with `version.workspace = true`.
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <semver>   e.g. $0 0.2.0" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# package.json — the first "version" key, which belongs to the package itself.
perl -0pi -e 's/("version"\s*:\s*")[^"]+(")/${1}'"$version"'${2}/' package.json

# Cargo workspace — the version under [workspace.package].
perl -0pi -e 's/(\[workspace\.package\]\nversion\s*=\s*")[^"]+(")/${1}'"$version"'${2}/' Cargo.toml

# Keep Cargo.lock in step so the commit is complete.
cargo metadata --format-version 1 >/dev/null

echo "Version set to $version:"
grep -m1 '"version"' package.json
grep -A1 '^\[workspace.package\]' Cargo.toml | grep version
echo
echo "Next: update CHANGELOG.md, commit, then tag v$version."
