#!/usr/bin/env bash
#
# Two binaries in this workspace, and for a while both were called `prune`: the desktop app
# (`src-tauri`) and the command line (`crates/prune-cli`). Cargo writes both to
# `target/<profile>/prune`, so whichever was built last won. `pnpm tauri build` followed by
# `./target/release/prune scan` launched the GUI and ignored the arguments — and a release built
# in one directory could have shipped the desktop app under the command line's name.
#
# Cargo does warn, but a warning in the middle of a five-minute build is not a check.
set -euo pipefail

names=$(cargo metadata --no-deps --format-version 1 |
  tr '{' '\n' | grep -o '"kind":\["bin"\][^}]*"name":"[^"]*"' |
  sed 's/.*"name":"\([^"]*\)".*/\1/' || true)

# Fall back to reading the manifests if the metadata shape ever changes, so this cannot pass
# by finding nothing.
if [ -z "$names" ]; then
  echo "FAIL: could not read any binary target from cargo metadata" >&2
  exit 1
fi

duplicates=$(echo "$names" | sort | uniq -d)
if [ -n "$duplicates" ]; then
  echo "FAIL: more than one binary target is called:" >&2
  echo "$duplicates" | sed 's/^/  /' >&2
  echo "" >&2
  echo "They would overwrite each other in target/<profile>/. Give each a distinct name." >&2
  exit 1
fi

echo "ok: $(echo "$names" | wc -l | tr -d ' ') binary targets, all distinctly named"
echo "$names" | sed 's/^/  /'
