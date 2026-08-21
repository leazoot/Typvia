#!/bin/zsh

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

# First desktop E2E scenario: the global command panel insertion loop, end to
# end against the real running app.
#
#   summon panel (⌘⇧V) → search → ↵ → assert the snippet text is delivered into
#   a third-party app (TextEdit), the original clipboard is restored, the panel
#   hides and returns focus, and usage is recorded (drives the Home ledger).
#
# Why a platform-native (osascript) harness and not tauri-driver/WebdriverIO:
# tauri-driver supports only Windows + Linux (macOS WKWebView has no WebDriver),
# and this flow is global-shortcut + cross-app injection — outside any
# WebView-DOM driver's reach on any platform.
#
# Opt-in, like the injector live tests: requires a macOS GUI session with
# Accessibility permission granted to the terminal driving it. Not run in
# headless CI. Runs the app via `tauri dev` (the built binary is blank under the
# production CSP — a separate packaging concern).
#
# Usage:  zsh apps/desktop/e2e/panel_insert.sh   (exit 0 = pass, 1 = fail)
set -u

ROOT="${0:A:h}/../../.."
ROOT="${ROOT:A}"
DB="$HOME/Library/Application Support/dev.typvia.desktop/typvia.db"
DEVLOG="$(mktemp -t typvia_e2e_dev)"
SENTINEL="TYPVIA_E2E_CLIP_SENTINEL"
MARK="TYPVIA_E2E_INSERT_OK"
SEED_ID="e2e-panel-insert"
TS=1785926400000

if [[ "$(uname)" != "Darwin" ]]; then
  echo "SKIP: panel insert E2E is macOS-only (platform-native automation)"
  exit 0
fi

fail() { echo "E2E FAIL: $1"; cleanup; exit 1; }

cleanup() {
  pkill -x typvia-desktop 2>/dev/null
  lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null
  osascript -e 'tell application "TextEdit" to quit saving no' >/dev/null 2>&1
  sqlite3 "$DB" "DELETE FROM snippet WHERE id='$SEED_ID'; DELETE FROM snippet_fts WHERE snippet_id='$SEED_ID';" 2>/dev/null
  rm -f "$DEVLOG"
}

# --- seed one searchable snippet through the real schema (app not running) ---
pkill -x typvia-desktop 2>/dev/null
lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null
sleep 1
[[ -f "$DB" ]] || fail "app database not found at $DB (run the app once first)"
sqlite3 "$DB" "DELETE FROM snippet WHERE id='$SEED_ID'; DELETE FROM snippet_fts WHERE snippet_id='$SEED_ID';"
sqlite3 "$DB" "INSERT INTO snippet (id,workspace_id,title,content_plaintext,type,security_level,platform_scope,created_at,updated_at,usage_count,version) VALUES ('$SEED_ID','default','E2E demo snippet','$MARK delivered by the panel','text','normal','[]',$TS,$TS,0,1);" || fail "seed insert failed"
sqlite3 "$DB" "INSERT INTO snippet_fts (snippet_id,title,content,description,tags,folder_name,\"trigger\",language) VALUES ('$SEED_ID','E2E demo snippet','$MARK delivered by the panel','','','','','');" || fail "seed fts insert failed"

# --- launch the app (tauri dev) ---
(cd "$ROOT" && nohup pnpm dev > "$DEVLOG" 2>&1 &)
for i in $(seq 1 45); do pgrep -x typvia-desktop >/dev/null 2>&1 && break; sleep 2; done
pgrep -x typvia-desktop >/dev/null 2>&1 || fail "app did not launch (see $DEVLOG)"
sleep 3

# --- prepare a third-party target ---
osascript -e 'tell application "TextEdit" to activate' -e 'tell application "TextEdit" to make new document' >/dev/null 2>&1
sleep 1
osascript -e 'tell application "TextEdit" to activate' >/dev/null 2>&1
sleep 0.6
osascript -e "set the clipboard to \"$SENTINEL\"" >/dev/null 2>&1

# --- summon → search → insert ---
osascript -e 'tell application "System Events" to key code 9 using {command down, shift down}'
sleep 1.0
osascript -e 'tell application "System Events" to keystroke "demo"'
sleep 1.0
osascript -e 'tell application "System Events" to key code 36'   # Return
sleep 2.0

DOC=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null)
CLIP=$(osascript -e 'the clipboard')
FRONT=$(osascript -e 'tell application "System Events" to name of first application process whose frontmost is true')
USAGE=$(sqlite3 "$DB" "SELECT usage_count FROM snippet WHERE id='$SEED_ID';")
LASTUSED=$(sqlite3 "$DB" "SELECT COALESCE(last_used_at,0) FROM snippet WHERE id='$SEED_ID';")

echo "delivered=$(echo "$DOC" | grep -q "$MARK" && echo yes || echo no) clipboard=$CLIP frontmost=$FRONT usage=$USAGE lastused=$LASTUSED"

echo "$DOC" | grep -q "$MARK" || fail "snippet text not delivered to TextEdit"
[[ "$CLIP" == "$SENTINEL" ]] || fail "original clipboard not restored (got: $CLIP)"
[[ "$FRONT" == "TextEdit" ]] || fail "focus not returned to the target app (frontmost: $FRONT)"
[[ "$USAGE" == "1" ]] || fail "usage not recorded once (usage_count=$USAGE)"
[[ "$LASTUSED" != "0" ]] || fail "last_used_at not set (Home ledger would miss it)"

echo "E2E PASS: panel insert loop delivered, clipboard restored, focus returned, usage recorded"
cleanup
exit 0
