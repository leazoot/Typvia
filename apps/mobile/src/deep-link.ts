// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Deep-link routing: exactly two controlled routes.
 * `typvia://search` lands on the search-first Home; `typvia://snippet/<id>`
 * opens that snippet, with an unknown or failing id falling back to Home
 * (never an error screen). Every URL is untrusted input — any app can fire
 * the scheme — so parse-and-validate comes first and anything outside the
 * two routes degrades to the plain open.
 */
import { getSnippet } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';

/** Snippet ids are UUIDs (core `new_id`); anything else is invalid. */
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export interface DeepLinkActions {
  /** Land on the search-first Home screen. */
  openSearch: () => void;
  /** Open one snippet in the editor. */
  openSnippet: (snippet: Snippet) => void;
}

/**
 * Builds the handler the deep-link plugin feeds opened URLs into. `load`
 * is injectable so tests mock at the IPC layer boundary.
 */
export function createDeepLinkHandler(
  actions: DeepLinkActions,
  load: (id: string) => Promise<Snippet> = getSnippet,
): (urls: string[]) => Promise<void> {
  return async (urls) => {
    const raw = urls[0];
    if (raw === undefined) return;
    // Engine-agnostic parsing: WHATWG URL host handling for non-special
    // schemes differs between engines (WebKit yields host "snippet",
    // Chromium yields an empty host with an opaque "//snippet/..." path —
    // observed on Android), so the route is split by hand.
    if (!/^typvia:\/\//i.test(raw)) return;
    const [route = '', tail = ''] = raw.slice('typvia://'.length).split(/[/?#]/, 2);
    if (route.toLowerCase() === 'snippet' && UUID_RE.test(tail)) {
      try {
        actions.openSnippet(await load(tail));
        return;
      } catch {
        // Unknown/deleted id: fall through to the search screen.
      }
    }
    // `typvia://search`, an invalid snippet route, or any other host all
    // land on Home — an app open must always end somewhere sane.
    actions.openSearch();
  };
}
