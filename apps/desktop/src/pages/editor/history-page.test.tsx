// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { HistoryPage } from './history-page';

const SNIPPET: Snippet = {
  id: 's-1',
  title: 'PR review prompt',
  body: 'intro\nfocus on correctness\noutro',
  snippetType: 'prompt',
  securityLevel: 'normal',
  description: null,
  folderId: null,
  trigger: ';prreview',
  triggerMode: 'delimiter',
  language: null,
  isFavorite: false,
  isPinned: false,
  isEnabled: true,
  createdAt: 1,
  updatedAt: 30,
  lastUsedAt: null,
  usageCount: 0,
  version: 3,
  deletedAt: null,
};

const getSnippet = vi.fn();
const historyList = vi.fn();
const historyGet = vi.fn();
const historyRestore = vi.fn();

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    getSnippet: (...args: unknown[]) => getSnippet(...args) as Promise<Snippet>,
    historyList: (...args: unknown[]) =>
      historyList(...args) as Promise<SharedModule.SnippetHistory>,
    historyGet: (...args: unknown[]) => historyGet(...args) as Promise<SharedModule.VersionBody>,
    historyRestore: (...args: unknown[]) => historyRestore(...args) as Promise<Snippet>,
  };
});

function renderHistory() {
  return render(
    <MemoryRouter initialEntries={['/editor/s-1/history']}>
      <Routes>
        <Route path="/editor/:id/history" element={<HistoryPage />} />
        <Route path="/editor/:id" element={<div>Editor landing</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

function mockThreeVersions() {
  getSnippet.mockResolvedValue(SNIPPET);
  historyList.mockResolvedValue({
    current: 3,
    entries: [
      { version: 3, title: 'PR review prompt', createdAt: 30 },
      { version: 2, title: 'PR review prompt', createdAt: 20 },
      { version: 1, title: 'PR review prompt', createdAt: 10 },
    ],
  });
  historyGet.mockImplementation((_id: unknown, version: unknown) =>
    Promise.resolve(
      version === 3
        ? { version: 3, title: 'PR review prompt', body: SNIPPET.body, createdAt: 30 }
        : {
            version,
            title: 'PR review prompt',
            body: 'intro\nlook for bugs\noutro',
            createdAt: 20,
          },
    ),
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(cleanup);

describe('HistoryPage', () => {
  it('lists versions newest first and diffs the newest older one by default', async () => {
    mockThreeVersions();
    renderHistory();

    expect(await screen.findByRole('heading', { name: 'v2 → v3' })).toBeTruthy();
    const rail = screen.getByRole('navigation', { name: 'Versions' });
    expect(rail.textContent).toContain('3 versions');
    expect(screen.getByText('current')).toBeTruthy();
    expect(screen.getByText('comparing')).toBeTruthy();

    // The changed line shows as a removed/added pair with −/+ glyphs.
    expect(screen.getByText('−')).toBeTruthy();
    expect(screen.getByText('+')).toBeTruthy();
    expect(screen.getByText('look for bugs')).toBeTruthy();
    expect(screen.getByText('focus on correctness')).toBeTruthy();
    expect(screen.getByText('2 lines changed')).toBeTruthy();

    // Restore states its forward-writing consequence instead of confirming.
    expect(screen.getByRole('button', { name: 'Restore v2' })).toBeTruthy();
    expect(screen.getByText('Restoring writes a new v4 — v3 is never lost.')).toBeTruthy();
  });

  it('restores forward and reports the new version', async () => {
    mockThreeVersions();
    historyRestore.mockImplementation(() => {
      // After the restore the list gains v4 as the new current.
      historyList.mockResolvedValue({
        current: 4,
        entries: [
          { version: 4, title: 'PR review prompt', createdAt: 40 },
          { version: 3, title: 'PR review prompt', createdAt: 30 },
          { version: 2, title: 'PR review prompt', createdAt: 20 },
          { version: 1, title: 'PR review prompt', createdAt: 10 },
        ],
      });
      return Promise.resolve({ ...SNIPPET, version: 4 });
    });
    renderHistory();

    fireEvent.click(await screen.findByRole('button', { name: 'Restore v2' }));

    expect(await screen.findByText('Restored as v4 — nothing was lost.')).toBeTruthy();
    expect(historyRestore).toHaveBeenCalledWith('s-1', 2);
    expect(screen.getByText('v4')).toBeTruthy();
  });

  it('shows no version data while a sensitive snippet is locked', async () => {
    getSnippet.mockResolvedValue({ ...SNIPPET, securityLevel: 'sensitive', body: null });
    historyList.mockRejectedValue({ code: 'permission_denied', message: 'unlock the vault first' });

    renderHistory();

    expect(
      await screen.findByText("This snippet's history is intact — unlock the Vault to read it."),
    ).toBeTruthy();
    expect(screen.queryByRole('navigation', { name: 'Versions' })).toBeNull();
    expect(screen.queryByText(/v\d/)).toBeNull();
  });

  it('explains an empty history instead of showing a blank diff', async () => {
    getSnippet.mockResolvedValue({ ...SNIPPET, version: 1 });
    historyList.mockResolvedValue({
      current: 1,
      entries: [{ version: 1, title: 'PR review prompt', createdAt: 10 }],
    });

    renderHistory();

    expect(
      await screen.findByText('Only one version so far — every later save will appear here.'),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Restore/ })).toBeNull();
  });
});
