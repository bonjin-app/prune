#!/usr/bin/env bash
#
# Prune promises it never talks to a network. This turns that promise into something CI can
# check, because a claim in a README is only as good as the last dependency bump.
#
# Two checks, because the two halves of the workspace have different obligations:
#
#   1. The engine (`prune-core`, `prune-cli`) must have nothing network-capable at all — not an
#      HTTP client, not a TLS stack, not even a socket library. It is the part that reads the
#      user's disk and decides what may be removed, and it is also what the CLI ships as a
#      single binary anyone can inspect.
#
#   2. The desktop app may carry what Tauri itself needs. Tauri brings `tokio` (its async
#      runtime) and, through it, `socket2` and `mio`, because `tauri-plugin-single-instance`
#      hands a second launch over to the first through a Unix domain socket or a named pipe —
#      local IPC, not a network. What it must never carry is an HTTP client or a TLS stack:
#      those exist for one purpose, and finding one means something started phoning home.
#
# Run it for whichever target you are shipping; CI runs it on macOS and Windows.
set -euo pipefail

target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

# Crates whose only reason to exist is speaking to a remote host.
readonly REMOTE='reqwest|hyper|hyper-util|hyper-tls|curl|curl-sys|ureq|isahc|surf|attohttpc|tungstenite|tokio-tungstenite|quinn|h2|native-tls|openssl|openssl-sys|rustls|rustls-pemfile|webpki|webpki-roots|trust-dns-resolver|hickory-resolver|hickory-proto'
# The above, plus anything that can open a socket at all.
readonly ANY_SOCKET="${REMOTE}|tokio|tokio-util|socket2|mio|ipnet"

fail=0

check() {
  local label="$1" pattern="$2"
  shift 2
  local found
  found=$(cargo tree "$@" --target "$target" --edges normal --prefix none --no-dedupe 2>/dev/null |
    awk '{print $1}' | sort -u | grep -Ex "$pattern" || true)
  if [ -n "$found" ]; then
    echo "FAIL: $label depends on:" >&2
    echo "$found" | sed 's/^/  /' >&2
    echo "" >&2
    echo "Prune does not talk to a network. If this is genuinely needed, it needs a decision," >&2
    echo "not a dependency bump — see docs/ARCHITECTURE.md." >&2
    for crate in $found; do
      echo "--- why $crate is here ---" >&2
      cargo tree --invert "$crate" --target "$target" --edges normal 2>/dev/null | head -12 >&2
    done
    fail=1
  else
    echo "ok: $label"
  fi
}

echo "checking for network dependencies (target: $target)"
check "the engine (prune-core, prune-cli)" "$ANY_SOCKET" -p prune-core -p prune-cli
check "the workspace" "$REMOTE" --workspace

exit "$fail"
