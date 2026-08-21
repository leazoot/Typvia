#!/bin/bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

# Fetches the pinned official espanso release and stages its macOS binary for
# bundling. The binary is an aggregate-distribution
# component (GPL-3.0): it is never linked, only spawned as a separate process.
#
# Output: vendor/espanso/espanso (universal Mach-O, ad-hoc re-signed) plus
# vendor/espanso/VERSION. vendor/ is gitignored; run this before
# scripts/release/macos_package.sh to produce a bundle with the engine inside.
#
# The upstream signature binds Espanso.app's Info.plist, so the standalone
# copy MUST be re-signed or the hardened runtime kills it on launch; the
# release chain re-signs the whole bundle (Developer ID) after bundling.
set -euo pipefail

ESPANSO_VERSION="v2.4.0"
DMG_URL="https://github.com/espanso/espanso/releases/download/${ESPANSO_VERSION}/Espanso-Mac-Universal.dmg"
# Pinned 2026-08-17 from the official release asset.
DMG_SHA256="aaf81d7573db785e5447b867e0f2f1d6f061ea9fa1756fda02f21b0402407669"
# Binary inside the dmg at Espanso.app/Contents/MacOS/espanso (pre re-sign).
BIN_SHA256="27be18570a3a45eb405021290dbe9333c5299eeeee5da4b6d116ab11231f28e5"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VENDOR="$ROOT/vendor/espanso"

if [[ -f "$VENDOR/espanso" && -f "$VENDOR/VERSION" ]] \
  && [[ "$(cat "$VENDOR/VERSION")" == "$ESPANSO_VERSION" ]]; then
  echo "espanso ${ESPANSO_VERSION} already staged at vendor/espanso — nothing to do"
  exit 0
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"; [[ -n "${MOUNT:-}" ]] && hdiutil detach "$MOUNT" -quiet 2>/dev/null || true' EXIT

echo "downloading ${DMG_URL}"
curl -sL --fail -o "$WORK/espanso.dmg" "$DMG_URL"
echo "${DMG_SHA256}  $WORK/espanso.dmg" | shasum -a 256 -c - >/dev/null \
  || { echo "FAIL: dmg checksum mismatch — refusing to stage"; exit 1; }

MOUNT="$(hdiutil attach -nobrowse -readonly "$WORK/espanso.dmg" \
  | awk -F'\t' '/\/Volumes\//{print $NF}' | tail -1)"
cp "$MOUNT/Espanso.app/Contents/MacOS/espanso" "$WORK/espanso"
hdiutil detach "$MOUNT" -quiet
MOUNT=""

echo "${BIN_SHA256}  $WORK/espanso" | shasum -a 256 -c - >/dev/null \
  || { echo "FAIL: binary checksum mismatch — refusing to stage"; exit 1; }

# Standalone copy needs its own signature (see header); ad-hoc here, the
# release chain replaces it with the Developer ID identity when signing the app.
xattr -c "$WORK/espanso"
codesign -s - --force "$WORK/espanso" >/dev/null 2>&1
"$WORK/espanso" --version >/dev/null \
  || { echo "FAIL: staged binary does not run"; exit 1; }

mkdir -p "$VENDOR"
mv "$WORK/espanso" "$VENDOR/espanso"
printf '%s' "$ESPANSO_VERSION" > "$VENDOR/VERSION"
echo "staged espanso ${ESPANSO_VERSION} → vendor/espanso/espanso"
