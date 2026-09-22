#!/usr/bin/env bash
# Points the Homebrew cask at a published release.
#
#   scripts/update-cask.sh 0.2.0
#   scripts/update-cask.sh 0.2.0 --publish
#
# A cask carries the checksum of the file it installs, so it can only be written after the
# release exists — `set-version.sh` cannot do it at the same time as the rest. Run this once
# the release is published.
#
# `--publish` then pushes the cask to bonjin-app/homebrew-tap, which is what `brew` actually
# reads. Without it the tap keeps serving the previous version, and the README's install
# command installs something older than the release that was just cut.
set -euo pipefail

version="${1:-}"
publish="${2:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <semver> [--publish]   e.g. $0 0.2.0 --publish" >&2
  exit 1
fi
if [[ -n "$publish" && "$publish" != "--publish" ]]; then
  echo "unknown argument: $publish (expected --publish)" >&2
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

if [[ "$publish" != "--publish" ]]; then
  echo "Not published. Re-run with --publish to push this to bonjin-app/homebrew-tap,"
  echo "or the tap will keep serving the previous version."
  exit 0
fi

tap="$tmp/homebrew-tap"
git clone -q --depth 1 https://github.com/bonjin-app/homebrew-tap.git "$tap"
cp "$cask" "$tap/Casks/prune.rb"

if git -C "$tap" diff --quiet; then
  echo "Tap already serves $version; nothing to push."
  exit 0
fi

git -C "$tap" add Casks/prune.rb
git -C "$tap" commit -q -m "chore: prune $version"
git -C "$tap" push -q origin HEAD:main
echo "Pushed to bonjin-app/homebrew-tap: prune $version"
