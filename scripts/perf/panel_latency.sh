#!/usr/bin/env bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

# Panel display latency: summon → first painted frame
# < 150ms. Uses the debug-only probe (panel.rs `panel_ready` prints
# PANEL_LATENCY_MS in debug builds; compiled out of release).
#
# This script drives a `pnpm tauri dev` instance: it sends ⌘⇧V via System
# Events, then extracts the probe lines from the dev process output.
#
# Prerequisites: no other Typvia/vite dev instance running; the invoking
# terminal has Accessibility permission (to send the key event).
#
# Usage: scripts/perf/panel_latency.sh [summons]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SUMMONS="${1:-3}"
LOG="$(mktemp -t typvia-panel-latency)"

cd "$ROOT/apps/desktop"
pnpm tauri dev >"$LOG" 2>&1 &
DEV_PID=$!
trap 'kill $DEV_PID >/dev/null 2>&1 || true; pkill -x typvia-desktop >/dev/null 2>&1 || true' EXIT

# Wait for the app process (dev builds compile first — allow a while).
for _ in $(seq 1 600); do
  pgrep -x typvia-desktop >/dev/null && break
  sleep 1
done
pgrep -x typvia-desktop >/dev/null || {
  echo "error: dev instance did not start; see $LOG" >&2
  exit 1
}
sleep 5

for i in $(seq 1 "$SUMMONS"); do
  # ⌘⇧V — the real global shortcut path, not a synthetic IPC call.
  osascript -e 'tell application "System Events" to key code 9 using {command down, shift down}'
  sleep 1
  # ESC hides the panel again for the next summon.
  osascript -e 'tell application "System Events" to key code 53'
  sleep 1
done

echo "probe lines:"
grep -o "PANEL_LATENCY_MS=[0-9.]*" "$LOG" | tail -n "$SUMMONS" || {
  echo "no probe output — check Accessibility permission and $LOG" >&2
  exit 1
}
