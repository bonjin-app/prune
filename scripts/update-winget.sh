#!/usr/bin/env bash
# Points the WinGet manifests at a published release.
#
#   scripts/update-winget.sh 0.2.0
#
# Like the Homebrew cask, a WinGet manifest carries the checksum of a file that does not exist
# until the release is published, so this runs after step 4 rather than with the version bump.
#
# It also re-reads the MSI's ProductCode. WiX generates a new one for every build, and a
# manifest carrying the previous release's code is worse than one carrying none: WinGet would
# match an installed copy against a code that is no longer there and get upgrade and uninstall
# wrong. Reading it needs olefile (`pip3 install --user olefile`); without it this stops rather
# than leaving the old code in place.
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <semver>   e.g. $0 0.2.0" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="$root/packaging/winget"
repo="bonjin-app/prune"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

exe="Prune_${version}_x64-setup.exe"
msi="Prune_${version}_x64_en-US.msi"
for f in "$exe" "$msi"; do
  url="https://github.com/$repo/releases/download/v$version/$f"
  echo "fetching $url"
  curl -fsSL --retry 3 -o "$tmp/$f" "$url"
done

# WinGet writes checksums in upper case.
exe_sha="$(shasum -a 256 "$tmp/$exe" | cut -d' ' -f1 | tr 'a-f' 'A-F')"
msi_sha="$(shasum -a 256 "$tmp/$msi" | cut -d' ' -f1 | tr 'a-f' 'A-F')"
product_code="$(python3 "$root/scripts/msi-property.py" "$tmp/$msi" ProductCode)"
released="$(date -u +%Y-%m-%d)"

for f in "$dir"/*.yaml; do
  perl -0pi -e 's/(PackageVersion: ).*/${1}'"$version"'/'                              "$f"
  perl -0pi -e 's/(ReleaseDate: ).*/${1}'"$released"'/'                                "$f"
  perl -0pi -e 's/(releases\/download\/v)[0-9][^\/]*/${1}'"$version"'/g'               "$f"
  perl -0pi -e 's/(releases\/tag\/v)[0-9].*/${1}'"$version"'/'                         "$f"
  perl -0pi -e 's/Prune_[0-9]+\.[0-9]+\.[0-9]+(?=_x64)/Prune_'"$version"'/g'           "$f"
  perl -0pi -e 's/(InstallerSha256: )[0-9A-F]{64}(?=\s*$)/${1}PLACEHOLDER/gm'          "$f"
done

# Two installers, two checksums: the first InstallerSha256 belongs to the .exe, the second to
# the .msi, in the order they appear under Installers.
perl -0pi -e 's/PLACEHOLDER/'"$exe_sha"'/'                                             "$dir/PruneContributors.Prune.installer.yaml"
perl -0pi -e 's/PLACEHOLDER/'"$msi_sha"'/'                                             "$dir/PruneContributors.Prune.installer.yaml"
perl -0pi -e "s/(ProductCode: ')[^']*(')/\${1}$product_code\${2}/"                     "$dir/PruneContributors.Prune.installer.yaml"

if grep -q PLACEHOLDER "$dir"/*.yaml; then
  echo "a checksum was not filled in; the manifest is left broken on purpose" >&2
  exit 1
fi

echo
echo "WinGet manifests updated:"
grep -hE '^(PackageVersion|ReleaseDate):| (InstallerSha256|ProductCode|InstallerUrl):' "$dir"/*.yaml |
  sort -u | sed 's/^/  /'
echo
echo "Next: validate on a Windows machine with"
echo "  winget validate --manifest packaging/winget"
echo "then submit with wingetcreate (see RELEASING.md)."
