// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { SearchHit, Snippet } from '@typvia/shared';
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
    title: `Title ${String(index)}`,
    body: `Body line ${String(index)}\nsecond line`,
    snippetType: 'text',
    securityLevel: 'normal',
    description: null,
    folderId: index % 2 === 0 ? 'f-1' : null,
    trigger: `/t${String(index)}`,
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: null,
    usageCount: 0,
    version: 1,
    deletedAt: null,
  };
}

const countSnippets = vi
  .fn<(view: string, folderId: string | null) => Promise<number>>()
  .mockResolvedValue(TOTAL);
type PageArgs = [string, string | null, string | null, number, number, string?];
const listSnippetPage = vi.fn((...args: PageArgs) => {
  const [, , , limit, offset] = args;
  return Promise.resolve(
    Array.from({ length: Math.min(limit, TOTAL - offset) }, (_, index) =>
      fakeSnippet(offset + index),
    ),
  );
});
const mainInsert = vi.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined);
const copySnippet = vi.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined);
const searchLibraryDeep = vi.fn((query: string) =>
  Promise.resolve(query.includes('zzz') ? [] : [fakeSnippet(7), fakeSnippet(21), fakeSnippet(35)]),
);
const searchSnippets = vi.fn((): Promise<SearchHit[]> =>
  Promise.resolve([{ snippetId: 's-21', title: 'Title 21', tier: 'trigger', isSensitive: false }]),
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
        folders: [{ folderId: 'f-1', count: 2104 }],
      }),
    countSnippets: (view: string, folderId: string | null) => countSnippets(view, folderId),
    listSnippetPage: (...args: PageArgs) => listSnippetPage(...args),
    searchLibraryDeep: (query: string) => searchLibraryDeep(query),
    searchSnippets: () => searchSnippets(),
    mainInsert: (id: string) => mainInsert(id),
    copySnippet: (id: string) => copySnippet(id),
    vaultList: () => Promise.resolve([fakeSnippet(1), fakeSnippet(2)]),
    syncStatus: () => Promise.reject(new Error('no host in tests')),
    templateVariables: (body: string) =>
      Promise.resolve(body.startsWith('Body line 1\n') ? ['date'] : []),
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [{ id: 'f-1', parentId: null, name: 'Infra', sortOrder: 0, createdAt: 1, updatedAt: 1 }]
          : [],
      ),
  };
});

function renderPage(locale: 'en' | 'zh' = 'en') {
  return render(
    <I18nProvider locale={locale}>
      <MemoryRouter initialEntries={['/']}>
        <Routes>
          <Route path="/" element={<LibraryPage />} />
          <Route path="/editor" element={<div>editor-new-stub</div>} />
          <Route path="/editor/:id" element={<div>editor-edit-stub</div>} />
          <Route path="/trash" element={<div>trash-stub</div>} />
        </Routes>
      </MemoryRouter>
    </I18nProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('LibraryPage — three columns', () => {
  it('virtualises 50k rows and shows the picked snippet in full', async () => {
    const { container } = renderPage();
    await screen.findByText('Body line 0');
    const rows = container.querySelectorAll('.tvl-row');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(100);
    expect(listSnippetPage).toHaveBeenCalledWith('all', null, null, 200, 0, 'recent');
    expect(screen.getByRole('heading', { level: 1, name: 'All snippets' })).toBeDefined();
    expect(screen.getByText('50,000', { selector: '.tvl-head-count' })).toBeDefined();

    const detail = screen.getByRole('region', { name: 'Snippet' });
    expect(within(detail).getByText('Infra · /t0')).toBeDefined();
    expect(within(detail).getByText(/Body line 0\s+second line/)).toBeDefined();
    expect(within(detail).getByText('Not used yet')).toBeDefined();
  });

  it('changes the list, title, count and detail together when a collection is chosen', async () => {
    renderPage();
    await screen.findByText('Body line 0');
    fireEvent.click(screen.getByRole('radio', { name: /Infra/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Infra' })).toBeDefined();
    expect(countSnippets).toHaveBeenLastCalledWith('folder', 'f-1');
    expect(screen.getByRole('radio', { name: /Infra/ }).getAttribute('aria-checked')).toBe('true');
  });

  it('asks the core for the chosen order instead of sorting on the page', async () => {
    renderPage();
    await screen.findByText('Body line 0');
    fireEvent.click(screen.getByRole('radio', { name: 'Most used' }));
    await waitFor(() =>
      expect(listSnippetPage).toHaveBeenLastCalledWith('all', null, null, 200, 0, 'used'),
    );
  });

  it('counts a snippet’s variables on its row and keeps inserting it to the panel', async () => {
    renderPage();
    expect(await screen.findByText('1 variable')).toBeDefined();
    fireEvent.click(screen.getByText('Body line 1'));
    const detail = screen.getByRole('region', { name: 'Snippet' });
    expect(await within(detail).findByText('1 · {{date}}')).toBeDefined();
    expect(within(detail).queryByRole('button', { name: /Insert into the front app/ })).toBeNull();
  });
});

describe('LibraryPage — inserting', () => {
  it('walks rows with the arrow keys and inserts the picked one on Enter', async () => {
    renderPage();
    await screen.findByText('Body line 0');
    const list = screen.getByTestId('library-viewport');
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(document.getElementById('tvl-row-2')?.getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(mainInsert).toHaveBeenCalledWith('s-2');
  });

  it('says first that nothing is lost when injection is not allowed, then offers copy', async () => {
    mainInsert.mockRejectedValueOnce({ code: 'permission_denied', message: 'not trusted' });
    renderPage();
    await screen.findByText('Body line 0');
    fireEvent.click(screen.getByRole('button', { name: /Insert into the front app/ }));
    const note = await screen.findByRole('status', { name: "Typvia can't type for you yet." });
    expect(within(note).getByText(/All 50,000 snippets are saved/)).toBeDefined();
    fireEvent.click(within(note).getByRole('button', { name: /copy it to the clipboard/ }));
    expect(copySnippet).toHaveBeenCalledWith('s-0');
  });
});

describe('LibraryPage — search', () => {
  it('groups results by where they matched and marks the match', async () => {
    const { container } = renderPage();
    await screen.findByText('Body line 0');
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'line' } });
    expect(await screen.findByText('In the text · 2')).toBeDefined();
    expect(screen.getByText('In the trigger · 1')).toBeDefined();
    expect(screen.getByText('3 · all collections')).toBeDefined();
    expect(container.querySelectorAll('mark.tvl-mark').length).toBeGreaterThan(0);
  });

  it('offers ways on with real numbers when nothing matches', async () => {
    renderPage();
    await screen.findByText('Body line 0');
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'zzz' } });
    expect(await screen.findByText('None of the 50,000 says this.')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Look in the trash · 3 there' }));
    expect(await screen.findByText('trash-stub')).toBeDefined();
  });
});

describe('LibraryPage — states and language', () => {
  it('invites the first snippet when the library is empty', async () => {
    countSnippets.mockResolvedValueOnce(0);
    renderPage();
    expect(await screen.findByText('Nothing of yours here yet.')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: /Write the first snippet/ }));
    expect(await screen.findByText('editor-new-stub')).toBeDefined();
  });

  it('renders single-language Chinese copy under locale zh', async () => {
    renderPage('zh');
    await screen.findByText('Body line 0');
    expect(screen.getByRole('radio', { name: /全部片段/ })).toBeDefined();
    expect(screen.getByRole('radio', { name: '最近用过' })).toBeDefined();
    expect(screen.getByText('50,000 条', { selector: '.tvl-head-count' })).toBeDefined();
    expect(screen.queryByText('All snippets')).toBeNull();
  });
});
