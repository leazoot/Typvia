// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Single-language rendering for the extension surfaces. The app's `useTr`
 * hook cannot run here, so the same inline en/zh pair contract keys off the
 * browser UI language instead.
 */

/** True when the browser UI language is Chinese. */
export function isZh(language: string): boolean {
  return language.toLowerCase().startsWith('zh');
}

/** Returns a translator over the given browser language. */
export function makeTr(language: string): (en: string, zh: string) => string {
  const zh = isZh(language);
  return (en, zhText) => (zh ? zhText : en);
}
