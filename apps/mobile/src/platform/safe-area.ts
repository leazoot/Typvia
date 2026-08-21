// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Safe-area sync for the Android host. iOS WebKit reports the real insets
 * through env(safe-area-inset-*); the Android WebView reports 0 while the
 * activity is edge-to-edge, so MainActivity.kt measures the system-bar +
 * cutout insets and exposes them via the `TypviaInsets` JS bridge. This
 * module mirrors them into `--tv-inset-top/bottom`, which the app CSS folds
 * into `--tv-safe-top/bottom` with max(env(…), var(…)).
 */
interface AndroidInsetsBridge {
  /** "top,bottom" in CSS px (the host divides by display density). */
  cssPx(): string;
}

declare global {
  interface Window {
    TypviaInsets?: AndroidInsetsBridge | undefined;
    /** Host push hook: MainActivity re-runs it whenever the insets change. */
    __TYPVIA_INSETS_CHANGED__?: (() => void) | undefined;
  }
}

/**
 * Reads the host insets once (covers reloads, where no inset dispatch will
 * come) and installs the change hook the host pushes through on rotation or
 * nav-mode changes. Without the bridge (iOS, desktop dev shell, tests) this
 * is a no-op and env() alone drives the safe areas.
 */
export function syncAndroidSafeArea(target: HTMLElement = document.documentElement): void {
  const bridge = window.TypviaInsets;
  if (bridge === undefined) return;
  const apply = () => {
    const [top, bottom] = bridge.cssPx().split(',').map(Number);
    setInsetVar(target, '--tv-inset-top', top);
    setInsetVar(target, '--tv-inset-bottom', bottom);
  };
  window.__TYPVIA_INSETS_CHANGED__ = apply;
  apply();
}

function setInsetVar(target: HTMLElement, name: string, value: number | undefined): void {
  // A malformed bridge answer leaves the previous value standing — never
  // write NaN into a padding calc.
  if (value !== undefined && Number.isFinite(value) && value >= 0) {
    target.style.setProperty(name, `${String(value)}px`);
  }
}
