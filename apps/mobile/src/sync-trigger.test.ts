// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SYNC_TRIGGER_COOLDOWN_MS, createSyncTrigger } from './sync-trigger';

/** Drives the document's visibility the way a foreground return does. */
function setVisibility(state: DocumentVisibilityState) {
  Object.defineProperty(document, 'visibilityState', { value: state, configurable: true });
  document.dispatchEvent(new Event('visibilitychange'));
}

afterEach(() => {
  setVisibility('visible');
  vi.clearAllMocks();
});

describe('mobile foreground sync trigger', () => {
  it('runs a round when the WebView comes back to the foreground', () => {
    const run = vi.fn().mockResolvedValue(undefined);
    const trigger = createSyncTrigger(run, () => 0);
    trigger.start();

    setVisibility('hidden');
    expect(run).not.toHaveBeenCalled();

    setVisibility('visible');
    expect(run).toHaveBeenCalledTimes(1);

    trigger.stop();
  });

  it('ignores a second foreground return inside the cooldown', async () => {
    const run = vi.fn().mockResolvedValue(undefined);
    let now = 1_000;
    const trigger = createSyncTrigger(run, () => now);
    trigger.start();

    setVisibility('visible');
    // Let the round settle so only the cooldown, not the in-flight guard,
    // decides what happens next.
    await Promise.resolve();
    await Promise.resolve();

    now += SYNC_TRIGGER_COOLDOWN_MS - 1;
    setVisibility('visible');
    expect(run).toHaveBeenCalledTimes(1);

    now += 2;
    setVisibility('visible');
    expect(run).toHaveBeenCalledTimes(2);

    trigger.stop();
  });

  it('stops listening and swallows an offline round', async () => {
    const run = vi.fn().mockRejectedValue(new Error('the server could not be reached'));
    const trigger = createSyncTrigger(run, () => 0);
    trigger.start();

    setVisibility('visible');
    await Promise.resolve();
    expect(run).toHaveBeenCalledTimes(1);

    trigger.stop();
    setVisibility('visible');
    expect(run).toHaveBeenCalledTimes(1);
  });
});
