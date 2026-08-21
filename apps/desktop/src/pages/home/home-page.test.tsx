// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { HomePage } from './home-page';

const NOW = Date.now();

function usedSnippet(index: number, usedAt: number, uses: number): Snippet {
  return {
    id: `s-${String(index)}`,
    title: `Used ${String(index)}`,
    body: `body ${String(index)}`,
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: null,
    trigger: `;u${String(index)}`,
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: usedAt,
    usageCount: uses,
    version: 1,
    deletedAt: null,
  };
}

let total = 12481;
let recentRows: Snippet[] = [];
const searchLibrary = vi.fn<(query: string, limit: number) => Promise<Snippet[]>>();
const copySnippet = vi.fn<(id: string) => Promise<void>>();
const trashSnippet = vi.fn<(id: string) => Promise<void>>();

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    libraryCounts: () =>
      Promise.resolve({
        total,
        recent: recentRows.length,
        starred: 0,
        unsorted: 0,
        trash: 3,
        folders: [],
      }),
    listSnippetPage: () => Promise.resolve(recentRows),
    searchLibrary: (query: string, limit: number) => searchLibrary(query, limit),
    copySnippet: (id: string) => copySnippet(id),
    trashSnippet: (id: string) => trashSnippet(id),
  };
});

/** Shows where a navigation landed, and what draft it seeded. */
function Landing({ name }: { name: string }) {
  const location = useLocation();
  return (
    <div>
      {name} {JSON.stringify(location.state ?? {})}
    </div>
  );
}

function renderHome() {
  return render(
    <MemoryRouter initialEntries={['/']}>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/library" element={<Landing name="library-stub" />} />
        <Route path="/editor" element={<Landing name="editor-stub" />} />
        <Route path="/editor/:id" element={<Landing name="editor-edit-stub" />} />
        <Route path="/vault" element={<Landing name="vault-stub" />} />
        <Route path="/ai" element={<Landing name="ai-stub" />} />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  total = 12481;
  recentRows = [];
  searchLibrary.mockResolvedValue([]);
});

describe('HomePage — the ledger', () => {
  it('lists what was reached for, with day and minute separators', async () => {
    recentRows = [
      usedSnippet(1, NOW - 60_000, 4),
      usedSnippet(2, NOW - 65_000, 2),
      usedSnippet(3, NOW - 26 * 60 * 60 * 1000, 6),
    ];
    renderHome();
    expect(await screen.findByText('Used 1')).toBeDefined();
    expect(screen.getByText('Today')).toBeDefined();
    expect(screen.getByText(/Yesterday|Earlier/)).toBeDefined();
    expect(screen.getByText('4×')).toBeDefined();
    // The ambient meta carries the count — no status rail, no dashboard.
    expect(screen.getByText('12,481 snippets')).toBeDefined();
    expect(screen.getByText('This Mac')).toBeDefined();
  });

  it('copies from the row and reports it on the button itself', async () => {
    recentRows = [usedSnippet(1, NOW - 60_000, 4)];
    copySnippet.mockResolvedValue();
    renderHome();
    const row = await screen.findByRole('button', { name: 'Used 1' });
    fireEvent.click(within(row).getByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(copySnippet).toHaveBeenCalledWith('s-1'));
    expect(await within(row).findByRole('button', { name: 'Copied' })).toBeDefined();
  });

  it('keeps deletion out of the row and behind the right-click menu', async () => {
    recentRows = [usedSnippet(1, NOW - 60_000, 4)];
    trashSnippet.mockImplementation((id) => {
      recentRows = recentRows.filter((row) => row.id !== id);
      return Promise.resolve();
    });
    renderHome();
    const row = await screen.findByRole('button', { name: 'Used 1' });
    expect(within(row).queryByRole('button', { name: /Delete|Trash/ })).toBeNull();

    fireEvent.contextMenu(row);
    const menu = screen.getByRole('menu', { name: 'Snippet actions' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items[items.length - 1]?.textContent).toBe('Move to Trash');
    fireEvent.click(items[items.length - 1]!);
    await waitFor(() => expect(trashSnippet).toHaveBeenCalledWith('s-1'));
    // The row leaves in place — no toast.
    await waitFor(() => expect(screen.queryByText('Used 1')).toBeNull());
  });
});

describe('HomePage — search → act', () => {
  it('lands a printable keystroke in the search line and replaces the ledger', async () => {
    recentRows = [usedSnippet(1, NOW - 60_000, 4)];
    searchLibrary.mockResolvedValue([usedSnippet(2, NOW, 1)]);
    renderHome();
    await screen.findByText('Used 1');

    fireEvent.keyDown(document, { key: 'e' });
    const field = screen.getByLabelText<HTMLInputElement>('Search snippets');
    expect(field.value).toBe('e');
    expect(await screen.findByText('Used 2')).toBeDefined();
    await waitFor(() => expect(screen.queryByText('Recent')).toBeNull());
    expect(searchLibrary).toHaveBeenCalledWith('e', 8);
  });

  it('offers a snippet command when nothing matches, and a trigger command for ":" queries', async () => {
    renderHome();
    await screen.findByText('Recent');
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'invoice' } });
    expect(await screen.findByText('No snippet matches that yet.')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Create a snippet' }));
    expect(await screen.findByText(/editor-stub \{"draftTitle":"invoice"\}/)).toBeDefined();
  });

  it('seeds the trigger when the query looks like one', async () => {
    renderHome();
    await screen.findByText('Recent');
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: ':email' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Create a snippet' }));
    expect(await screen.findByText(/editor-stub \{"draftTrigger":":email"\}/)).toBeDefined();
  });

  it('opens quick create with three kinds', async () => {
    renderHome();
    fireEvent.click(await screen.findByRole('button', { name: 'New' }));
    const menu = screen.getByRole('menu', { name: 'New' });
    expect(within(menu).getAllByRole('menuitem')).toHaveLength(3);
    fireEvent.click(within(menu).getByRole('menuitem', { name: /Secret/ }));
    expect(await screen.findByText(/vault-stub \{"create":true\}/)).toBeDefined();
  });
});

describe('HomePage — empty and language', () => {
  it('is quiet, not empty, before the first snippet', async () => {
    total = 0;
    renderHome();
    expect(await screen.findByText('It is quiet here.')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Create the first snippet' }));
    expect(await screen.findByText(/editor-stub/)).toBeDefined();
  });

  it('renders single-language Chinese copy under locale zh', async () => {
    render(
      <I18nProvider locale="zh">
        <MemoryRouter initialEntries={['/']}>
          <Routes>
            <Route path="/" element={<HomePage />} />
          </Routes>
        </MemoryRouter>
      </I18nProvider>,
    );
    expect(await screen.findByText('12,481 个片段')).toBeDefined();
    expect(screen.getByText('找点什么?')).toBeDefined();
    expect(screen.getByText('本机')).toBeDefined();
    expect(screen.queryByText('This Mac')).toBeNull();
  });
});
