#!/usr/bin/env bash

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

# Reproducible TypviaMobileFFI build: compiles the typvia-mobile-ffi static
# library for iOS device + simulator, generates the
# Swift bindings in library mode (no UDL), assembles an xcframework, and runs
# the host Swift smoke against the same generated bindings.
#
# Outputs land in target/mobile-ffi/ (build products, never committed).
# Suitable as an Xcode pre-build phase once the keyboard extension target
# consumes the xcframework.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/target/mobile-ffi"
GEN="$OUT/generated"
LIB=libtypvia_mobile_ffi.a

cd "$ROOT"

echo "==> Building iOS static libraries (release)"
# Pin the C-object minimum to the product baseline (iOS 16.0, project.yml)
# so linking the keyboard extension raises no min-version warnings.
export IPHONEOS_DEPLOYMENT_TARGET=16.0
cargo build -p typvia-mobile-ffi --release --target aarch64-apple-ios
cargo build -p typvia-mobile-ffi --release --target aarch64-apple-ios-sim
cargo build -p typvia-mobile-ffi --release --target x86_64-apple-ios

echo "==> Building host library (bindgen metadata + smoke link)"
cargo build -p typvia-mobile-ffi --release

rm -rf "$OUT"
mkdir -p "$GEN" "$OUT/device" "$OUT/simulator" "$OUT/include"

echo "==> Generating Swift bindings (library mode)"
cargo run -p typvia-mobile-ffi --bin uniffi-bindgen --features uniffi/cli --release -- \
  generate --library "$ROOT/target/release/libtypvia_mobile_ffi.dylib" \
  --language swift --out-dir "$GEN"

# Device slice is arm64; the simulator slice must hold arm64 + x86_64.
cp "$ROOT/target/aarch64-apple-ios/release/$LIB" "$OUT/device/$LIB"
lipo -create \
  "$ROOT/target/aarch64-apple-ios-sim/release/$LIB" \
  "$ROOT/target/x86_64-apple-ios/release/$LIB" \
  -output "$OUT/simulator/$LIB"

# xcodebuild expects a clang module: header + module.modulemap side by side.
cp "$GEN/typvia_mobile_ffiFFI.h" "$OUT/include/"
cp "$GEN/typvia_mobile_ffiFFI.modulemap" "$OUT/include/module.modulemap"

echo "==> Assembling xcframework"
rm -rf "$OUT/TypviaMobileFFI.xcframework"
xcodebuild -create-xcframework \
  -library "$OUT/device/$LIB" -headers "$OUT/include" \
  -library "$OUT/simulator/$LIB" -headers "$OUT/include" \
  -output "$OUT/TypviaMobileFFI.xcframework"

echo "==> Host Swift smoke (typed calls through the generated bindings)"
swiftc -O -o "$OUT/ffi-smoke" \
  "$ROOT/crates/mobile-ffi/smoke/main.swift" \
  "$GEN/typvia_mobile_ffi.swift" \
  -I "$GEN" \
  -Xcc -fmodule-map-file="$GEN/typvia_mobile_ffiFFI.modulemap" \
  "$ROOT/target/release/libtypvia_mobile_ffi.a"
"$OUT/ffi-smoke"

echo "==> Done: $OUT/TypviaMobileFFI.xcframework"
