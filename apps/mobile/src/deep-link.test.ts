// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it, vi } from 'vitest';
import { createDeepLinkHandler } from './deep-link';
import type { Snippet } from '@typvia/shared';

const SNIPPET = { id: '4c8e2c1a-9a1b-4f5e-8a2d-1b2c3d4e5f60', title: 'T' } as Snippet;

function actions() {
  return { openSearch: vi.fn(), openSnippet: vi.fn() };
}

describe('deep-link routing', () => {
  it('routes typvia://search to the search screen', async () => {
    const acts = actions();
    await createDeepLinkHandler(acts, vi.fn())(['typvia://search']);
    expect(acts.openSearch).toHaveBeenCalledTimes(1);
    expect(acts.openSnippet).not.toHaveBeenCalled();
  });

  it('opens a snippet for a valid id', async () => {
    const acts = actions();
    const load = vi.fn().mockResolvedValue(SNIPPET);
    await createDeepLinkHandler(acts, load)([`typvia://snippet/${SNIPPET.id}`]);
    expect(load).toHaveBeenCalledWith(SNIPPET.id);
    expect(acts.openSnippet).toHaveBeenCalledWith(SNIPPET);
    expect(acts.openSearch).not.toHaveBeenCalled();
  });

  it('falls back to search for an unknown id (load fails)', async () => {
    const acts = actions();
    const load = vi.fn().mockRejectedValue(new Error('not found'));
    await createDeepLinkHandler(acts, load)([`typvia://snippet/${SNIPPET.id}`]);
    expect(acts.openSearch).toHaveBeenCalledTimes(1);
    expect(acts.openSnippet).not.toHaveBeenCalled();
  });

  it('rejects a malformed id without touching IPC', async () => {
    const acts = actions();
    const load = vi.fn();
    await createDeepLinkHandler(acts, load)(['typvia://snippet/../etc/passwd']);
    expect(load).not.toHaveBeenCalled();
    expect(acts.openSearch).toHaveBeenCalledTimes(1);
  });

  it('parses without relying on URL host semantics (Chromium opaque-path regression)', async () => {
    // Chromium WebViews parse `typvia://snippet/x` with an empty host and
    // an opaque path; the router must not depend on `new URL().host`.
    const acts = actions();
    const load = vi.fn().mockResolvedValue(SNIPPET);
    await createDeepLinkHandler(acts, load)([`TYPVIA://SNIPPET/${SNIPPET.id}`]);
    expect(load).toHaveBeenCalledWith(SNIPPET.id);
    expect(acts.openSnippet).toHaveBeenCalledTimes(1);
  });

  it('ignores foreign schemes and unparseable urls entirely', async () => {
    const acts = actions();
    await createDeepLinkHandler(acts, vi.fn())(['https://evil.example/x']);
    await createDeepLinkHandler(acts, vi.fn())(['not a url']);
    await createDeepLinkHandler(acts, vi.fn())([]);
    expect(acts.openSearch).not.toHaveBeenCalled();
    expect(acts.openSnippet).not.toHaveBeenCalled();
  });

  it('lands unknown typvia hosts on search (an open always ends somewhere)', async () => {
    const acts = actions();
    await createDeepLinkHandler(acts, vi.fn())(['typvia://insert/abc']);
    expect(acts.openSearch).toHaveBeenCalledTimes(1);
  });
});
