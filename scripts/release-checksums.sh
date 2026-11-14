#!/usr/bin/env bash
# Writes SHA256SUMS for every file attached to a GitHub release, and attaches it with --upload.
#
#   scripts/release-checksums.sh v0.2.0            # print it, change nothing
#   scripts/release-checksums.sh v0.2.0 --upload   # also attach it to the release
#
# The checksums are taken from the files as GitHub serves them, not from a local build, so they
# describe exactly what a person downloads. Check a download with:
#
#   shasum -a 256 --check --ignore-missing SHA256SUMS      # macOS
#   sha256sum --check --ignore-missing SHA256SUMS          # Linux, Git Bash
set -euo pipefail

tag="${1:?usage: release-checksums.sh <tag> [--upload]}"
upload="${2:-}"
repo="${GITHUB_REPOSITORY:-bonjin-app/prune}"

if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum "$@"; }
else
  sha() { shasum -a 256 "$@"; }
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Everything except an earlier SHA256SUMS, which would otherwise list itself.
gh release download "$tag" --repo "$repo" --dir "$work/assets" --pattern '*'
rm -f "$work/assets/SHA256SUMS"

count="$(find "$work/assets" -type f | wc -l | tr -d ' ')"
if [ "$count" -eq 0 ]; then
  echo "error: release $tag has no files" >&2
  exit 1
fi

# Sorted by name, so the file reads the same however the release listed its assets.
(
  cd "$work/assets"
  find . -maxdepth 1 -type f -print | sed 's|^\./||' | LC_ALL=C sort |
    while IFS= read -r name; do sha "$name"; done
) >"$work/SHA256SUMS"
# Proves the file reads back before anyone else relies on it.
(cd "$work/assets" && sha --check --quiet "$work/SHA256SUMS")

cat "$work/SHA256SUMS"
echo "($count files)" >&2

if [ "$upload" = "--upload" ]; then
  gh release upload "$tag" --repo "$repo" "$work/SHA256SUMS" --clobber
  echo "attached SHA256SUMS to $tag" >&2
elif [ -n "$upload" ]; then
  echo "error: unknown option $upload" >&2
  exit 2
fi
