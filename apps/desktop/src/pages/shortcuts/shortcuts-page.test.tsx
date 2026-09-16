// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, render, screen, within } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { ShortcutsPage } from './shortcuts-page';

const summonShortcut = vi.fn();

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, summonShortcut: () => summonShortcut() as Promise<string | null> };
});

afterEach(cleanup);

describe('ShortcutsPage', () => {
  it('lists each action once with its macOS symbols and Windows words', () => {
    summonShortcut.mockResolvedValue(null);
    render(<ShortcutsPage />);
    const table = screen.getByRole('table', { name: 'Shortcuts' });
    const summon = within(table)
      .getByRole('rowheader', { name: 'Summon Quick Bar' })
      .closest('[role="row"]');
    if (!(summon instanceof HTMLElement)) throw new Error('no summon row');
    expect(within(summon).getByText('⌘⇧V')).toBeDefined();
    expect(within(summon).getByText('Ctrl Alt V')).toBeDefined();
    const headers = within(table)
      .getAllByRole('rowheader')
      .map((cell) => cell.textContent);
    expect(new Set(headers).size).toBe(headers.length);
  });

  it('names the summon key the system actually granted', async () => {
    summonShortcut.mockResolvedValue('Ctrl Alt V');
    render(<ShortcutsPage />);
    expect(await screen.findByText('in any app · Ctrl Alt V here')).toBeDefined();
  });

  it('says so when no summon key was granted', async () => {
    summonShortcut.mockResolvedValue(null);
    render(<ShortcutsPage />);
    expect(await screen.findByText('this machine granted none')).toBeDefined();
  });

  it('speaks one language under zh', () => {
    summonShortcut.mockResolvedValue(null);
    render(
      <I18nProvider locale="zh">
        <ShortcutsPage />
      </I18nProvider>,
    );
    expect(screen.getByRole('heading', { name: '快捷键总览' })).toBeDefined();
    expect(screen.queryByText('Every shortcut')).toBeNull();
  });
});
