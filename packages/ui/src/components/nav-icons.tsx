// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Bottom-navigation line icons (design: typvia_mobile_export, 20px grid,
 * 1.4px strokes with a 1.7px inner accent stroke). Stroke colour rides on
 * `currentColor`, so the active/inactive state is pure CSS `color`.
 */

export function HomeNavIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <rect
        x="3.4"
        y="3.4"
        width="13.2"
        height="13.2"
        rx="3.8"
        stroke="currentColor"
        strokeWidth="1.4"
      />
      <path d="M10 7.2v5.6" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  );
}

export function LibraryNavIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path
        d="M3.6 5.6h12.8M3.6 10h9.2M3.6 14.4h11"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function VaultNavIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path
        d="M10 3.2l5.3 2v4.5c0 3.4-2.2 5.6-5.3 6.9-3.1-1.3-5.3-3.5-5.3-6.9V5.2l5.3-2z"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path d="M10 8.1v3.3" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  );
}

export function SettingsNavIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <path
        d="M3.6 7.4h12.8M3.6 12.6h12.8"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
      <path
        d="M12.6 5.5v3.8M7 10.7v3.8"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
      />
    </svg>
  );
}
