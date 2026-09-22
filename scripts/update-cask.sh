#!/usr/bin/env bash
# Points the Homebrew cask at a published release.
#
#   scripts/update-cask.sh 0.2.0
#
# A cask carries the checksum of the file it installs, so it can only be written after the
# release exists — `set-version.sh` cannot do it at the same time as the rest. Run this once
# the release is published, then copy the cask to the tap (see RELEASING.md).
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <semver>   e.g. $0 0.2.0" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cask="$root/packaging/homebrew/Casks/prune.rb"
repo="bonjin-app/prune"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Checksum what people will actually download, rather than a local build that happens to carry
# the same version number.
for a in aarch64 x64; do
  url="https://github.com/$repo/releases/download/v$version/Prune_${version}_${a}.dmg"
  echo "fetching $url"
  curl -fsSL --retry 3 -o "$tmp/$a.dmg" "$url"
done

arm="$(shasum -a 256 "$tmp/aarch64.dmg" | cut -d' ' -f1)"
intel="$(shasum -a 256 "$tmp/x64.dmg" | cut -d' ' -f1)"

perl -0pi -e 's/(version\s+")[^"]+(")/${1}'"$version"'${2}/'                  "$cask"
perl -0pi -e 's/(sha256 arm:\s+")[0-9a-f]{64}(")/${1}'"$arm"'${2}/'           "$cask"
perl -0pi -e 's/(\s+intel:\s+")[0-9a-f]{64}(")/${1}'"$intel"'${2}/'           "$cask"

ruby -c "$cask" >/dev/null

echo
echo "Cask updated:"
grep -E '^ +(version |sha256 arm:|intel:)' "$cask" | sed 's/^/  /'
echo
echo "Next: copy packaging/homebrew/Casks/prune.rb into the bonjin-app/homebrew-tap repository."
