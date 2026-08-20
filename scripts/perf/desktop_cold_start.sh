#!/usr/bin/env bash
# Cold start: target < 1.5s from launch to a visible
# main window. Compiles the CGWindowList probe (cold_start_probe.swift —
# polls window *owner names* only; no accessibility or screen-recording
# permission needed) and runs it against a cold process N times.
#
# Usage: scripts/perf/desktop_cold_start.sh [path-to-Typvia.app] [runs]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP="${1:-$ROOT/target/release/bundle/macos/Typvia.app}"
RUNS="${2:-3}"
PROBE="${TMPDIR:-/tmp}/typvia-cold-start-probe"

if [[ ! -d "$APP" ]]; then
  echo "error: $APP not found — run 'pnpm tauri build --bundles app' first" >&2
  exit 1
fi

if [[ ! -x "$PROBE" || "$ROOT/scripts/perf/cold_start_probe.swift" -nt "$PROBE" ]]; then
  swiftc -O -o "$PROBE" "$ROOT/scripts/perf/cold_start_probe.swift"
fi

quit_app() {
  # SIGTERM rather than an Apple event: an unattended session can leave the
  # quit event un-serviced for its full two-minute timeout.
  pkill -x typvia-desktop 2>/dev/null || true
  for _ in $(seq 1 20); do
    pgrep -x typvia-desktop >/dev/null || { sleep 1; return 0; }
    sleep 0.2
  done
  pkill -9 -x typvia-desktop 2>/dev/null || true
  sleep 1
}

for run in $(seq 1 "$RUNS"); do
  quit_app
  MS="$("$PROBE" "$APP")"
  echo "run $run: ${MS}ms (budget 1500ms)"
done
quit_app
