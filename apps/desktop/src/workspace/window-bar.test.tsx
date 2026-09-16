// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { WindowBar } from './window-bar';

const summonShortcut = vi.fn();

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, summonShortcut: () => summonShortcut() as Promise<string | null> };
});

const minimize = vi.fn(() => Promise.resolve());
const toggleMaximize = vi.fn(() => Promise.resolve());
const close = vi.fn(() => Promise.resolve());

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ minimize, toggleMaximize, close }),
}));

function Probe() {
  return <p>at {useLocation().pathname}</p>;
}

function renderBar(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <WindowBar />
      <Routes>
        <Route path="*" element={<Probe />} />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

beforeEach(() => {
  summonShortcut.mockResolvedValue('Ctrl Alt V');
});

describe('WindowBar', () => {
  it('goes straight to a place and marks the one the window is in', async () => {
    renderBar('/');
    expect(screen.getByRole('button', { name: 'Snippets' }).getAttribute('aria-current')).toBe(
      'page',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Trash' }));
    expect(await screen.findByText('at /trash')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Trash' }).getAttribute('aria-current')).toBe('page');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('offers the way back from a page other than the library', () => {
    renderBar('/settings');
    expect(screen.getByRole('button', { name: 'Back' })).toBeDefined();
  });

  it('names the summon key the host actually registered', async () => {
    renderBar('/');
    expect(await screen.findByText('Ctrl Alt V')).toBeDefined();
  });

  it('offers no summon key when the system granted none', async () => {
    summonShortcut.mockResolvedValue(null);
    renderBar('/');
    expect(await screen.findByRole('button', { name: 'Minimize' })).toBeDefined();
    expect(screen.queryByText('Summon')).toBeNull();
  });

  it('works the window from its own three buttons', () => {
    renderBar('/');
    fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Maximize' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(minimize).toHaveBeenCalledTimes(1);
    expect(toggleMaximize).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledTimes(1);
  });
});
