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

# The README links straight at the release files, and their names carry the version. Left
# behind, those links 404 the moment the next release lands — `releases/latest/download/<name>`
# needs the exact name, and there is no unversioned alias to point at instead.
perl -0pi -e 's/Prune_\d+\.\d+\.\d+(?=_(?:aarch64|x64)(?:-setup|_en-US)?\.(?:dmg|exe|msi))/Prune_'"$version"'/g' README.md
perl -0pi -e 's/prune-v\d+\.\d+\.\d+(?=-<target>)/prune-v'"$version"'/g' README.md

# Keep Cargo.lock in step so the commit is complete.
cargo metadata --format-version 1 >/dev/null

echo "Version set to $version:"
grep -m1 '"version"' package.json
grep -A1 '^\[workspace.package\]' Cargo.toml | grep version
grep -c "Prune_$version" README.md | sed 's/^/README download links: /'
echo
echo "Next: update CHANGELOG.md, commit, then tag v$version."
