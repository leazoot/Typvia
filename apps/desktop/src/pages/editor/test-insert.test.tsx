// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { IpcError } from '@typvia/shared';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { TestInsert } from './test-insert';

const injectSnippet = vi.fn(() => Promise.resolve());
const copySnippet = vi.fn(() => Promise.resolve());

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    injectSnippet: (...args: Parameters<typeof injectSnippet>) => injectSnippet(...args),
    copySnippet: (...args: Parameters<typeof copySnippet>) => copySnippet(...args),
  };
});

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  vi.clearAllMocks();
});

/** Advances fake timers by `ms` and flushes the resulting microtasks. */
async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('TestInsert', () => {
  it('is disabled until the saved content is ready', () => {
    render(<TestInsert snippetId={null} ready={false} />);
    expect(screen.getByRole('button', { name: 'Test insert' }).hasAttribute('disabled')).toBe(true);
  });

  it('counts down, then injects the saved snippet', async () => {
    render(<TestInsert snippetId="s-1" ready />);
    fireEvent.click(screen.getByRole('button', { name: 'Test insert' }));

    // Countdown runs before anything is injected.
    expect(screen.getByText(/Focus your target app… 3/)).toBeDefined();
    expect(injectSnippet).not.toHaveBeenCalled();

    await advance(3000);
    expect(injectSnippet).toHaveBeenCalledWith('s-1');
    expect(screen.getByText('Inserted')).toBeDefined();
  });

  it('can be cancelled mid-countdown without injecting', async () => {
    render(<TestInsert snippetId="s-1" ready />);
    fireEvent.click(screen.getByRole('button', { name: 'Test insert' }));
    await advance(1000);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await advance(3000);
    expect(injectSnippet).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Test insert' })).toBeDefined();
  });

  it('falls back to copy when injection lacks OS permission', async () => {
    injectSnippet.mockRejectedValueOnce(new IpcError('permission_denied', 'no permission'));
    render(<TestInsert snippetId="s-1" ready />);
    fireEvent.click(screen.getByRole('button', { name: 'Test insert' }));
    await advance(3000);
    expect(copySnippet).toHaveBeenCalledWith('s-1');
    expect(screen.getByText('Copied instead')).toBeDefined();
  });
});
