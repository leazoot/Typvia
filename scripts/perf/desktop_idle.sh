#!/usr/bin/env bash
# Idle metrics: idle memory < 120MB, idle CPU ≈ 0%.
# Launches the built Typvia.app, lets it settle, samples RSS and %CPU, then
# quits it. Build the bundle first: `pnpm tauri build --bundles app`.
#
# Usage: scripts/perf/desktop_idle.sh [path-to-Typvia.app]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP="${1:-$ROOT/target/release/bundle/macos/Typvia.app}"
SETTLE_S=15
SAMPLES=10
INTERVAL_S=2

if [[ ! -d "$APP" ]]; then
  echo "error: $APP not found — run 'pnpm tauri build --bundles app' first" >&2
  exit 1
fi
if pgrep -x typvia-desktop >/dev/null; then
  echo "error: a typvia-desktop process is already running — quit it first" >&2
  exit 1
fi

open "$APP"
for _ in $(seq 1 50); do
  pgrep -x typvia-desktop >/dev/null && break
  sleep 0.2
done
PID="$(pgrep -x typvia-desktop | head -1)"
if [[ -z "$PID" ]]; then
  echo "error: Typvia did not start" >&2
  exit 1
fi
echo "settling ${SETTLE_S}s (pid $PID)…"
sleep "$SETTLE_S"

MAX_RSS_KB=0
CPU_SUM=0
for i in $(seq 1 "$SAMPLES"); do
  read -r RSS_KB CPU <<<"$(ps -o rss=,%cpu= -p "$PID")"
  CPU="${CPU%\%}"
  echo "sample $i: rss $((RSS_KB / 1024))MB cpu ${CPU}%"
  ((RSS_KB > MAX_RSS_KB)) && MAX_RSS_KB=$RSS_KB
  CPU_SUM="$(python3 -c "print($CPU_SUM + $CPU)")"
  sleep "$INTERVAL_S"
done

osascript -e 'tell application "Typvia" to quit' >/dev/null 2>&1 || kill "$PID"

MAX_MB=$((MAX_RSS_KB / 1024))
AVG_CPU="$(python3 -c "print(round($CPU_SUM / $SAMPLES, 2))")"
echo "max rss: ${MAX_MB}MB (budget 120MB), avg idle cpu: ${AVG_CPU}%"
if ((MAX_MB >= 120)); then
  echo "OVER BUDGET"
  exit 1
fi
echo "OK"
