#!/bin/zsh
# Template fill-and-inject E2E, end to end against the real app.
#
#   summon panel (⌘⇧V) → search a template → ↵ (enter fill mode) → type a field
#   value → ↵ → assert the RENDERED text ({{module}} replaced) is delivered into
#   TextEdit, and usage is recorded (drives the Home ledger).
#
# Same platform-native rationale and opt-in requirements as panel_insert.sh
# (macOS GUI session + Accessibility permission; runs via `tauri dev`). Not run
# in headless CI.
#
# Usage:  zsh apps/desktop/e2e/template_fill.sh   (exit 0 = pass, 1 = fail)
set -u

ROOT="${0:A:h}/../../.."
ROOT="${ROOT:A}"
DB="$HOME/Library/Application Support/dev.typvia.desktop/typvia.db"
DEVLOG="$(mktemp -t typvia_e2e_tpl)"
MARK="TYPVIA_E2E_TPL_OK"
SEED_ID="e2e-panel-template"
FIELD_ID="e2e-panel-template-field"
TS=1785926400000

if [[ "$(uname)" != "Darwin" ]]; then
  echo "SKIP: template fill E2E is macOS-only (platform-native automation)"
  exit 0
fi

fail() { echo "E2E FAIL: $1"; cleanup; exit 1; }

cleanup() {
  pkill -x typvia-desktop 2>/dev/null
  lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null
  osascript -e 'tell application "TextEdit" to quit saving no' >/dev/null 2>&1
  sqlite3 "$DB" "DELETE FROM template_field WHERE id='$FIELD_ID'; DELETE FROM snippet WHERE id='$SEED_ID'; DELETE FROM snippet_fts WHERE snippet_id='$SEED_ID';" 2>/dev/null
  rm -f "$DEVLOG"
}

# --- seed a template snippet + one required field through the real schema ---
pkill -x typvia-desktop 2>/dev/null
lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null
sleep 1
[[ -f "$DB" ]] || fail "app database not found at $DB (run the app once first)"
sqlite3 "$DB" "DELETE FROM template_field WHERE id='$FIELD_ID'; DELETE FROM snippet WHERE id='$SEED_ID'; DELETE FROM snippet_fts WHERE snippet_id='$SEED_ID';"
sqlite3 "$DB" "INSERT INTO snippet (id,workspace_id,title,content_plaintext,type,security_level,platform_scope,created_at,updated_at,usage_count,version) VALUES ('$SEED_ID','default','E2E tpltest template','$MARK m={{module}}','template','normal','[]',$TS,$TS,0,1);" || fail "seed snippet failed"
sqlite3 "$DB" "INSERT INTO snippet_fts (snippet_id,title,content,description,tags,folder_name,\"trigger\",language) VALUES ('$SEED_ID','E2E tpltest template','$MARK m={{module}}','','','','','');" || fail "seed fts failed"
sqlite3 "$DB" "INSERT INTO template_field (id,snippet_id,name,label,type,default_value,options,validation,is_required,sort_order,platform_overrides) VALUES ('$FIELD_ID','$SEED_ID','module','Module','single_line_text',NULL,'[]',NULL,1,0,NULL);" || fail "seed field failed"

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

# --- summon → search → ↵ (fill mode) → type value → ↵ (render + inject) ---
osascript -e 'tell application "System Events" to key code 9 using {command down, shift down}'
sleep 1.0
osascript -e 'tell application "System Events" to keystroke "tpltest"'
sleep 1.0
osascript -e 'tell application "System Events" to key code 36'   # Return → enter fill mode
sleep 1.6
osascript -e 'tell application "System Events" to keystroke "auth"'
sleep 0.8
osascript -e 'tell application "System Events" to key code 36'   # Return → render + inject
sleep 2.0

DOC=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null)
USAGE=$(sqlite3 "$DB" "SELECT usage_count FROM snippet WHERE id='$SEED_ID';")
LASTUSED=$(sqlite3 "$DB" "SELECT COALESCE(last_used_at,0) FROM snippet WHERE id='$SEED_ID';")
EXPECTED="$MARK m=auth"

echo "delivered=$(echo "$DOC" | grep -q "$EXPECTED" && echo yes || echo no) doc=\"$DOC\" usage=$USAGE lastused=$LASTUSED"

echo "$DOC" | grep -q "$EXPECTED" || fail "rendered template not delivered (expected '$EXPECTED')"
echo "$DOC" | grep -q "{{module}}" && fail "placeholder was injected unrendered"
[[ "$USAGE" == "1" ]] || fail "usage not recorded once (usage_count=$USAGE)"
[[ "$LASTUSED" != "0" ]] || fail "last_used_at not set (Home ledger would miss it)"

echo "E2E PASS: template filled and rendered text delivered, usage recorded"
cleanup
exit 0
