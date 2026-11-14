#!/usr/bin/env bash
# Checks a release the way a person who downloads it will meet it, before anyone publishes it.
#
#   scripts/verify-release.sh 0.2.2
#
# Works on a draft as well as a published release. macOS only, since it opens the disk images
# and asks macOS what it thinks of the app inside. It changes nothing: it downloads into a
# temporary directory and reads.
#
# What it checks, and why each one is here:
#   - every file named in SHA256SUMS matches, and every file attached is named there;
#   - the command line in the macOS archive reports the version being released;
#   - each disk image holds Prune.app at that version, built for the architecture in its name;
#   - that app's signature verifies with `codesign --verify --deep --strict`. 0.1.6 and 0.2.0
#     shipped with only the linker's signature on the executable, which macOS reports as
#     "damaged" on a downloaded copy, with no way to allow it. CI was green for both;
#   - `syspolicy_check distribution` reports nothing fatal except the missing notarization,
#     which is expected until the release is signed with a Developer ID.
set -euo pipefail

version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <semver>   e.g. $0 0.2.2" >&2
  exit 1
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: this opens macOS disk images; run it on a Mac" >&2
  exit 1
fi

repo="${GITHUB_REPOSITORY:-bonjin-app/prune}"
tag="v$version"
work="$(mktemp -d)"
mounts=()
cleanup() {
  for m in "${mounts[@]:-}"; do [ -n "$m" ] && hdiutil detach -quiet "$m" 2>/dev/null || true; done
  rm -rf "$work"
}
trap cleanup EXIT

failures=0
pass() { echo "  ok    $*"; }
fail() {
  echo "  FAIL  $*"
  failures=$((failures + 1))
}

echo "Release $tag of $repo"
gh release download "$tag" --repo "$repo" --dir "$work" --pattern '*'
cd "$work"

echo "Checksums"
if [ ! -f SHA256SUMS ]; then
  fail "no SHA256SUMS attached"
else
  if shasum -a 256 --check --quiet SHA256SUMS; then
    pass "$(wc -l <SHA256SUMS | tr -d ' ') files match SHA256SUMS"
  else
    fail "a file does not match SHA256SUMS"
  fi
  while IFS= read -r name; do
    grep -q "  $name\$" SHA256SUMS || fail "$name is attached but not in SHA256SUMS"
  done < <(find . -maxdepth 1 -type f ! -name SHA256SUMS -print | sed 's|^\./||' | LC_ALL=C sort)
fi

echo "Command line"
cli="prune-v$version-aarch64-apple-darwin.tar.gz"
if [ -f "$cli" ]; then
  mkdir cli && tar -xzf "$cli" -C cli
  reported="$(cli/prune --version 2>/dev/null || true)"
  if [ "$reported" = "prune $version" ]; then
    pass "$cli reports $reported"
  else
    fail "$cli reports '$reported'"
  fi
else
  fail "$cli is missing"
fi

echo "macOS app"
for pair in "aarch64:arm64" "x64:x86_64"; do
  name="${pair%%:*}"
  arch="${pair##*:}"
  dmg="Prune_${version}_${name}.dmg"
  if [ ! -f "$dmg" ]; then
    fail "$dmg is missing"
    continue
  fi
  mount="$(hdiutil attach -nobrowse -readonly "$dmg" 2>/dev/null | awk -F'\t' '/\/Volumes\//{print $NF}')"
  mounts+=("$mount")
  app="$mount/Prune.app"
  if [ ! -d "$app" ]; then
    fail "$dmg holds no Prune.app"
    continue
  fi

  bundled="$(defaults read "$app/Contents/Info.plist" CFBundleShortVersionString 2>/dev/null || true)"
  if [ "$bundled" = "$version" ]; then
    pass "$dmg: version $bundled"
  else
    fail "$dmg: version '$bundled'"
  fi

  archs="$(lipo -archs "$app/Contents/MacOS/prune-desktop" 2>/dev/null || true)"
  if [ "$archs" = "$arch" ]; then
    pass "$dmg: built for $archs"
  else
    fail "$dmg: built for '$archs', expected $arch"
  fi

  if codesign --verify --deep --strict "$app" 2>/dev/null; then
    pass "$dmg: signature verifies"
  else
    fail "$dmg: signature does not verify; macOS will call a downloaded copy damaged"
  fi

  # Fatal findings other than notarization, which an unsigned release cannot have.
  fatal="$(syspolicy_check distribution "$app" 2>&1 |
    awk '/Severity: Fatal/{getline; print}' | grep -v 'Notarization ticket' || true)"
  if [ -z "$fatal" ]; then
    pass "$dmg: nothing fatal to Gatekeeper but notarization"
  else
    fail "$dmg: Gatekeeper finds: $(echo "$fatal" | sed 's/^ *//' | head -1)"
  fi

  hdiutil detach -quiet "$mount" && mounts=("${mounts[@]/$mount/}")
done

echo
if [ "$failures" -eq 0 ]; then
  echo "$tag is ready to publish."
else
  echo "$tag has $failures problem(s). Do not publish it." >&2
  exit 1
fi
