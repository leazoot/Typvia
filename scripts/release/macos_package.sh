#!/usr/bin/env bash
# macOS distribution chain: build → bundle managed
# engine → sign → notarize → .dmg, verifying what the environment made
# possible and saying so honestly.
#
# Signing/notarization environment (nothing secret lives in this script):
#   APPLE_SIGNING_IDENTITY   "Developer ID Application: …"  (codesign)
#   APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID                (notarytool)
#     — or APPLE_API_KEY / APPLE_API_ISSUER / APPLE_API_KEY_PATH
# Without them the chain still produces an installable-by-override .dmg and
# reports each missing stage as PENDING (waiting on certificates), never as
# a pass. Credentials come from the user's keychain/CI secrets — never from
# this repository.
#
# Engine bundling: the pinned official espanso binary is staged by
# scripts/release/fetch_espanso.sh into vendor/ and injected into the .app
# together with its GPL-3.0 licence set; the .dmg is created from the final
# injected bundle. A failed staging (offline machine) is reported as PENDING.
#
# Usage: scripts/release/macos_package.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SIGNED=false
NOTARIZE=false
[[ -n "${APPLE_SIGNING_IDENTITY:-}" ]] && SIGNED=true
if [[ -n "${APPLE_ID:-}" || -n "${APPLE_API_KEY:-}" ]]; then NOTARIZE=true; fi

echo "== engine staging =="
ENGINE=false
if bash "$ROOT/scripts/release/fetch_espanso.sh"; then
  ENGINE=true
else
  echo "engine: PENDING — fetch_espanso.sh failed (offline?); bundle ships without the managed engine"
fi

echo "== build (signing: $SIGNED, notarization: $NOTARIZE) =="
cd "$ROOT/apps/desktop"
pnpm tauri build --bundles app

APP="$ROOT/target/release/bundle/macos/Typvia.app"
[[ -d "$APP" ]] || { echo "FAIL: no .app produced" >&2; exit 1; }
echo "app: $APP"

STATUS=0

echo "== engine injection =="
# The engine ships as a nested helper app with its own bundle id and
# LSUIElement: a bare binary in Contents/MacOS makes the engine's Cocoa
# runtime register with LaunchServices AS Typvia (second dock icon, and
# `open` refuses to relaunch the main app while the engine lives).
HELPER="$APP/Contents/Helpers/TypviaEngine.app"
# A previous chain run may have left the old bare-binary layout in the bundle.
rm -f "$APP/Contents/MacOS/espanso"
if $ENGINE; then
  LICDIR="$APP/Contents/Resources/licenses/espanso"
  mkdir -p "$LICDIR" "$HELPER/Contents/MacOS"
  cp "$ROOT/vendor/espanso/espanso" "$HELPER/Contents/MacOS/espanso"
  cat > "$HELPER/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.typvia.engine</string>
  <key>CFBundleName</key><string>Typvia Engine</string>
  <key>CFBundleExecutable</key><string>espanso</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST
  cp "$ROOT/vendor/espanso/VERSION" "$LICDIR/ESPANSO_VERSION"
  cp "$ROOT/licenses/espanso/GPL-3.0.txt" "$LICDIR/GPL-3.0.txt"
  cp "$ROOT/THIRD_PARTY_NOTICES.md" "$LICDIR/THIRD_PARTY_NOTICES.md"
  codesign -s - --force --identifier dev.typvia.engine "$HELPER" >/dev/null 2>&1
  if "$HELPER/Contents/MacOS/espanso" --version >/dev/null 2>&1; then
    echo "engine: OK ($(cat "$ROOT/vendor/espanso/VERSION") helper app + GPL-3.0 licence set)"
  else
    echo "engine: FAIL (injected binary does not run)"
    STATUS=1
  fi
else
  echo "engine: PENDING (not staged)"
fi

echo "== sign =="
# Without a Developer ID, fall back to the stable local dev identity
# (scripts/release/macos_dev_cert.sh): TCC accessibility grants and keychain
# ACLs bind to the signature, so an ad-hoc rebuild silently breaks
# injection/expansion until the user re-grants.
DEV_IDENTITY="Typvia Dev Signing"
if ! $SIGNED && security find-identity -v -p codesigning 2>/dev/null | grep -Fq "$DEV_IDENTITY"; then
  $ENGINE && codesign --force -s "$DEV_IDENTITY" "$HELPER"
  [[ -f "$APP/Contents/MacOS/typvia-browser-host" ]] \
    && codesign --force -s "$DEV_IDENTITY" "$APP/Contents/MacOS/typvia-browser-host"
  codesign --force -s "$DEV_IDENTITY" "$APP"
  echo "codesign: dev identity (stable local signature; Developer ID still PENDING)"
fi
if $SIGNED; then
  # Injection invalidated the bundler's signature; re-sign inside-out so the
  # nested engine helper carries the same Developer ID + hardened runtime.
  if $ENGINE; then
    codesign --force --options runtime -s "$APPLE_SIGNING_IDENTITY" "$HELPER" \
      || { echo "codesign(engine): FAIL"; STATUS=1; }
  fi
  codesign --force --options runtime -s "$APPLE_SIGNING_IDENTITY" "$APP" \
    || { echo "codesign(app): FAIL"; STATUS=1; }
  codesign --verify --deep --strict "$APP" && echo "codesign: OK" || { echo "codesign: FAIL"; STATUS=1; }
else
  echo "codesign: PENDING — set APPLE_SIGNING_IDENTITY (Developer ID certificate)"
fi

echo "== dmg =="
DMG="$ROOT/target/release/bundle/dmg/Typvia_$(date +%Y%m%d)_macos.dmg"
mkdir -p "$(dirname "$DMG")"
STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/Typvia.app"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Typvia" -srcfolder "$STAGE" -ov -quiet -format UDZO "$DMG"
rm -rf "$STAGE"
echo "dmg: $DMG"

echo "== notarize =="
if $NOTARIZE && $SIGNED; then
  if [[ -n "${APPLE_API_KEY:-}" ]]; then
    xcrun notarytool submit "$DMG" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" \
      --issuer "$APPLE_API_ISSUER" --wait || { echo "notarize: FAIL"; STATUS=1; }
  else
    xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" \
      --team-id "$APPLE_TEAM_ID" --wait || { echo "notarize: FAIL"; STATUS=1; }
  fi
  xcrun stapler staple "$DMG" && echo "staple: OK" || { echo "staple: FAIL"; STATUS=1; }
else
  echo "notarize: PENDING — set notarization env (and signing identity)"
fi

echo "== verify =="
if $SIGNED; then
  if spctl --assess --type execute "$APP" 2>/dev/null; then
    echo "gatekeeper: OK (accepted)"
  elif $NOTARIZE; then
    echo "gatekeeper: FAIL (signed + notarization env set but not accepted)"
    STATUS=1
  else
    echo "gatekeeper: PENDING (signed but not notarized)"
  fi
else
  echo "gatekeeper: PENDING — unsigned builds need right-click → Open on other machines"
fi
MOUNT="$(hdiutil attach -nobrowse -readonly "$DMG" | awk -F'\t' '/\/Volumes\//{print $NF}' | tail -1)"
if [[ -d "$MOUNT/Typvia.app" ]]; then
  echo "dmg contains Typvia.app: OK"
  if $ENGINE && [[ ! -f "$MOUNT/Typvia.app/Contents/Helpers/TypviaEngine.app/Contents/MacOS/espanso" ]]; then
    echo "dmg engine missing: FAIL"
    STATUS=1
  fi
else
  echo "dmg contents unexpected under $MOUNT"
  STATUS=1
fi
hdiutil detach "$MOUNT" -quiet || true

if [[ $STATUS -eq 0 ]]; then
  if $SIGNED && $NOTARIZE && $ENGINE; then
    echo "RESULT: distributable (signed + notarized + engine bundled)"
  else
    echo "RESULT: chain ready — PENDING stages listed above"
  fi
else
  echo "RESULT: FAILED"
fi
exit $STATUS
