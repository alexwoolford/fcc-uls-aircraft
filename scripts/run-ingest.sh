#!/usr/bin/env bash
# Oneshot: ingest FCC ULS complete aircraft zip into work sqlite.
# No published current/ copy.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${FCC_ULS_BIN:-$ROOT/bin/fcc-uls-aircraft}"
STATE="${FCC_ULS_STATE:-/var/lib/fcc-uls-aircraft}"
DB="${FCC_ULS_SQLITE:-$STATE/fcc-uls-aircraft.sqlite}"
LOCK="${FCC_ULS_LOCK:-$STATE/.ingest.lock}"

test -x "$BIN" || {
  echo "missing $BIN — build with: cargo build --release" >&2
  exit 1
}

mkdir -p "$STATE"

acquire_lock() {
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$LOCK"
    if ! flock -n 9; then
      echo "ingest already running (lock $LOCK)" >&2
      exit 1
    fi
  else
    if ! mkdir "$LOCK.d" 2>/dev/null; then
      echo "ingest already running (lock $LOCK.d)" >&2
      exit 1
    fi
    trap 'rmdir "$LOCK.d" 2>/dev/null || true' EXIT
  fi
}
acquire_lock

echo "== fcc-uls-aircraft ingest =="
echo "bin=$BIN db=$DB"
CACHE="${FCC_ULS_CACHE:-}"
if [[ -n "$CACHE" ]]; then
  mkdir -p "$CACHE"
  "$BIN" --db "$DB" ingest --cache-dir "$CACHE"
else
  "$BIN" --db "$DB" ingest
fi
