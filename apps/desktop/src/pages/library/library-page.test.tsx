// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { LibraryPage } from './library-page';

const TOTAL = 50_000;

function fakeSnippet(index: number): Snippet {
  return {
    id: `s-${String(index)}`,
    title: `Snippet ${String(index)}`,
    body: `body ${String(index)}`,
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: index % 2 === 0 ? 'f-1' : null,
    trigger: `;t${String(index)}`,
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: null,
    usageCount: 3,
    version: 1,
    deletedAt: null,
  };
}

const countSnippets = vi.fn(() => Promise.resolve(TOTAL));
const batchMoveSnippets = vi.fn(() => Promise.resolve());
const batchTagSnippets = vi.fn(() => Promise.resolve());
const batchTrashSnippets = vi.fn(() => Promise.resolve());
const createFolder = vi.fn(() => Promise.resolve({}));
const updateFolder = vi.fn(() => Promise.resolve({}));
const deleteFolder = vi.fn(() => Promise.resolve());
const createTag = vi.fn(() => Promise.resolve({}));
const renameTag = vi.fn(() => Promise.resolve());
const deleteTag = vi.fn(() => Promise.resolve());
const copySnippet = vi.fn(() => Promise.resolve());
const createSnippet = vi.fn(() => Promise.resolve({}));
const updateSnippet = vi.fn(() => Promise.resolve({}));
const trashSnippet = vi.fn(() => Promise.resolve());
const snippetConvertToSensitive = vi.fn(() => Promise.resolve());
const searchLibrary = vi.fn((query: string) =>
  Promise.resolve(
    query.includes('zzz')
      ? []
      : [fakeSnippet(7), fakeSnippet(21), fakeSnippet(35)].map((snippet, index) => ({
          ...snippet,
          title: `Hit ${String(index)}`,
        })),
  ),
);
const listSnippetPage = vi.fn(
  (_view: string, _folderId: string | null, _type: string | null, limit: number, offset: number) =>
    Promise.resolve(
      Array.from({ length: Math.min(limit, TOTAL - offset) }, (_, index) =>
        fakeSnippet(offset + index),
      ),
    ),
);

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    libraryCounts: () =>
      Promise.resolve({
        total: TOTAL,
        recent: 128,
        starred: 41,
        unsorted: 14,
        trash: 3,
        folders: [
          { folderId: 'f-1', count: 2104 },
          { folderId: 'f-2', count: 12 },
        ],
      }),
    countSnippets: (...args: Parameters<typeof countSnippets>) => countSnippets(...args),
    listSnippetPage: (...args: Parameters<typeof listSnippetPage>) => listSnippetPage(...args),
    searchLibraryDeep: (...args: Parameters<typeof searchLibrary>) => searchLibrary(...args),
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [
              {
                id: 'f-1',
                parentId: null,
                name: 'Infra',
                sortOrder: 0,
                createdAt: 1,
                updatedAt: 1,
              },
              {
                id: 'f-2',
                parentId: null,
                name: 'Support',
                sortOrder: 1,
                createdAt: 1,
                updatedAt: 1,
              },
            ]
          : [],
      ),
    batchMoveSnippets: (...args: Parameters<typeof batchMoveSnippets>) =>
      batchMoveSnippets(...args),
    batchTagSnippets: (...args: Parameters<typeof batchTagSnippets>) => batchTagSnippets(...args),
    batchTrashSnippets: (...args: Parameters<typeof batchTrashSnippets>) =>
      batchTrashSnippets(...args),
    createFolder: (...args: Parameters<typeof createFolder>) => createFolder(...args),
    updateFolder: (...args: Parameters<typeof updateFolder>) => updateFolder(...args),
    deleteFolder: (...args: Parameters<typeof deleteFolder>) => deleteFolder(...args),
    createTag: (...args: Parameters<typeof createTag>) => createTag(...args),
    renameTag: (...args: Parameters<typeof renameTag>) => renameTag(...args),
    deleteTag: (...args: Parameters<typeof deleteTag>) => deleteTag(...args),
    copySnippet: (...args: Parameters<typeof copySnippet>) => copySnippet(...args),
    createSnippet: (...args: Parameters<typeof createSnippet>) => createSnippet(...args),
    updateSnippet: (...args: Parameters<typeof updateSnippet>) => updateSnippet(...args),
    trashSnippet: (...args: Parameters<typeof trashSnippet>) => trashSnippet(...args),
    snippetConvertToSensitive: (...args: Parameters<typeof snippetConvertToSensitive>) =>
      snippetConvertToSensitive(...args),
    listTags: () => Promise.resolve([{ id: 'tag-1', name: 'prod', createdAt: 1 }]),
  };
});

function renderPage() {
  return render(
    <MemoryRouter initialEntries={['/library']}>
      <Routes>
        <Route path="/library" element={<LibraryPage />} />
        <Route path="/editor" element={<div>editor-new-stub</div>} />
        <Route path="/editor/:id" element={<div>editor-edit-stub</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('LibraryPage — browsing', () => {
  it('virtualises 50k rows and keeps the complexity folded away', async () => {
    const { container } = renderPage();
    await screen.findByText('Snippet 0');
    const rows = container.querySelectorAll('.tvl-row');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(100);
    expect(listSnippetPage).toHaveBeenCalledWith('all', null, null, 200, 0);
    // Nothing but the three views is exposed before the user asks.
    expect(screen.queryByRole('button', { name: 'Command' })).toBeNull();
    expect(screen.queryByRole('navigation', { name: 'Library' })).toBeNull();
    expect(screen.getByRole('tab', { name: 'All' })).toBeDefined();
  });

  it('narrows by type from the filter popover and shows a removable chip', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('button', { name: /Filter/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Command' }));
    expect(countSnippets).toHaveBeenLastCalledWith('all', null, 'command');

    fireEvent.click(screen.getByRole('button', { name: 'Remove type filter' }));
    expect(countSnippets).toHaveBeenLastCalledWith('all', null, null);
  });

  it('switches views from the text tabs and folders from the popover', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('tab', { name: 'Starred' }));
    expect(countSnippets).toHaveBeenLastCalledWith('starred', null, null);

    fireEvent.click(screen.getByRole('button', { name: /Filter/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Infra' }));
    expect(countSnippets).toHaveBeenLastCalledWith('folder', 'f-1', null);
  });

  it('opens a row in place and edits from the peek', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Snippet 2' }));
    const peek = screen.getByRole('region', { name: 'Details' });
    expect(within(peek).getByText('body 2')).toBeDefined();
    expect(within(peek).getByText(';t2')).toBeDefined();
    fireEvent.click(within(peek).getByRole('button', { name: 'Edit' }));
    expect(await screen.findByText('editor-edit-stub')).toBeDefined();
  });

  it('walks rows with the arrow keys', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText('Search snippets');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Snippet 0');
    fireEvent.keyDown(document.activeElement!, { key: 'ArrowDown' });
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Snippet 1');
  });
});

describe('LibraryPage — row actions', () => {
  it('copies from the row and confirms on the button', async () => {
    renderPage();
    const row = await screen.findByRole('button', { name: 'Snippet 1' });
    fireEvent.click(within(row).getByRole('button', { name: 'Copy' }));
    expect(copySnippet).toHaveBeenCalledWith('s-1');
    expect(await within(row).findByRole('button', { name: 'Copied' })).toBeDefined();
  });

  it('keeps duplicate, vault and delete behind ··· , with delete last', async () => {
    renderPage();
    const row = await screen.findByRole('button', { name: 'Snippet 1' });
    fireEvent.click(within(row).getByRole('button', { name: 'More actions' }));
    // The menu leaves the scrolling list on purpose — the list would clip it
    // (anchored-menu.ts) — so it is queried from the document, not the row.
    const menu = screen.getByRole('menu');
    expect(document.querySelector('.tvl-list')?.contains(menu)).toBe(false);
    const items = within(menu)
      .getAllByRole('menuitem')
      .map((item) => item.textContent);
    expect(items).toEqual(['Duplicate', 'Star', 'Move to Vault', 'Move to Trash']);
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Move to Vault' }));
    expect(snippetConvertToSensitive).toHaveBeenCalledWith('s-1');
  });

  it('offers the same actions on right-click', async () => {
    renderPage();
    const row = await screen.findByRole('button', { name: 'Snippet 1' });
    fireEvent.contextMenu(row);
    const menu = screen.getByRole('menu', { name: 'Snippet actions' });
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Move to Trash' }));
    expect(trashSnippet).toHaveBeenCalledWith('s-1');
  });

  it('stars a row without leaving it', async () => {
    renderPage();
    const row = await screen.findByRole('button', { name: 'Snippet 1' });
    fireEvent.click(within(row).getByRole('button', { name: '☆' }));
    expect(updateSnippet).toHaveBeenCalledWith(
      expect.objectContaining({ id: 's-1', isFavorite: true }),
    );
  });
});

describe('LibraryPage — search and selection', () => {
  it('replaces results on every keystroke', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText('Search snippets');
    fireEvent.change(input, { target: { value: 'd' } });
    fireEvent.change(input, { target: { value: 'do' } });
    expect(searchLibrary).toHaveBeenCalledTimes(2);
    expect(searchLibrary).toHaveBeenLastCalledWith('do', 500);

    expect(await screen.findByText('Hit 0')).toBeDefined();
    expect(screen.queryByText('Snippet 0')).toBeNull();
    expect(screen.getByText('3 results')).toBeDefined();
  });

  it('offers two real actions when nothing matches', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText<HTMLInputElement>('Search snippets');
    fireEvent.change(input, { target: { value: 'zzz' } });
    expect(await screen.findByText('No snippet matches that')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(input.value).toBe('');
    expect(await screen.findByText('Snippet 0')).toBeDefined();

    fireEvent.change(input, { target: { value: 'zzz again' } });
    await screen.findByText('No snippet matches that');
    fireEvent.click(screen.getByRole('button', { name: 'Save as snippet' }));
    expect(await screen.findByText('editor-new-stub')).toBeDefined();
  });

  it('raises the floating strip on ⌘-click, never on a permanent checkbox', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    expect(screen.queryByRole('checkbox')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Snippet 0' }), { metaKey: true });
    fireEvent.click(screen.getByRole('button', { name: 'Snippet 1' }), { metaKey: true });
    const strip = screen.getByRole('group', { name: 'Selection' });
    expect(within(strip).getByText('2 selected')).toBeDefined();

    fireEvent.click(within(strip).getByRole('button', { name: 'Move' }));
    fireEvent.click(within(strip).getByRole('menuitem', { name: 'Infra' }));
    expect(batchMoveSnippets).toHaveBeenCalledWith(['s-0', 's-1'], 'f-1');
    await waitFor(() => expect(screen.queryByText('2 selected')).toBeNull());
  });

  it('extends a selection with ⇧-click and asks with the real number', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('button', { name: 'Snippet 0' }), { metaKey: true });
    fireEvent.click(screen.getByRole('button', { name: 'Snippet 2' }), { shiftKey: true });
    const strip = screen.getByRole('group', { name: 'Selection' });
    expect(within(strip).getByText('3 selected')).toBeDefined();

    fireEvent.click(within(strip).getByRole('button', { name: 'Delete' }));
    fireEvent.click(within(strip).getByRole('button', { name: 'Delete 3 snippets' }));
    expect(batchTrashSnippets).toHaveBeenCalledWith(['s-0', 's-1', 's-2']);
  });

  it('tags a selection from the strip', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('button', { name: 'Snippet 0' }), { metaKey: true });
    const strip = screen.getByRole('group', { name: 'Selection' });
    fireEvent.click(within(strip).getByRole('button', { name: 'Tag' }));
    fireEvent.click(within(strip).getByRole('menuitem', { name: 'prod' }));
    expect(batchTagSnippets).toHaveBeenCalledWith(['s-0'], 'tag-1');
  });
});

describe('LibraryPage — folders and tags', () => {
  it('keeps organisation folded away and still creates, renames and deletes', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('button', { name: 'View options' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Folders & tags' }));
    const panel = await screen.findByRole('region', { name: 'Folders & tags' });

    fireEvent.click(within(panel).getAllByRole('button', { name: 'New' })[0]!);
    const input = within(panel).getByLabelText('New folder name');
    fireEvent.change(input, { target: { value: 'Notes' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(createFolder).toHaveBeenCalledWith({ name: 'Notes', parentId: null, sortOrder: 2 });

    fireEvent.click(within(panel).getByRole('button', { name: /Infra/ }));
    fireEvent.click(within(panel).getByRole('button', { name: 'Rename' }));
    const rename = within(panel).getByLabelText('Rename Infra');
    fireEvent.change(rename, { target: { value: 'Infrastructure' } });
    fireEvent.keyDown(rename, { key: 'Enter' });
    expect(updateFolder).toHaveBeenCalledWith({
      id: 'f-1',
      name: 'Infrastructure',
      parentId: null,
      sortOrder: 0,
    });

    fireEvent.click(within(panel).getAllByRole('button', { name: 'New' })[1]!);
    const tagInput = within(panel).getByLabelText('New tag name');
    fireEvent.change(tagInput, { target: { value: 'urgent' } });
    fireEvent.keyDown(tagInput, { key: 'Enter' });
    expect(createTag).toHaveBeenCalledWith('urgent');
  });
});

describe('LibraryPage — language', () => {
  it('renders single-language Chinese copy under locale zh', async () => {
    render(
      <I18nProvider locale="zh">
        <MemoryRouter initialEntries={['/library']}>
          <Routes>
            <Route path="/library" element={<LibraryPage />} />
          </Routes>
        </MemoryRouter>
      </I18nProvider>,
    );
    expect(await screen.findByRole('heading', { level: 1, name: '片段库' })).toBeDefined();
    expect(screen.getByRole('tab', { name: '全部' })).toBeDefined();
    expect(screen.getByLabelText('搜索片段')).toBeDefined();
    expect(screen.queryByRole('tab', { name: 'All' })).toBeNull();
  });
});
