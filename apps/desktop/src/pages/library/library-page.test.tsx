// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
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
const searchLibrary = vi.fn((query: string) =>
  Promise.resolve(
    query.includes('zzz')
      ? []
      : [fakeSnippet(7), fakeSnippet(21), fakeSnippet(35)].map((s, i) => ({
          ...s,
          title: `Hit ${String(i)}`,
        })),
  ),
);
const listSnippetPage = vi.fn(
  (_view: string, _folderId: string | null, _type: string | null, limit: number, offset: number) =>
    Promise.resolve(
      Array.from({ length: Math.min(limit, TOTAL - offset) }, (_, i) => fakeSnippet(offset + i)),
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
        folders: [{ folderId: 'f-1', count: 2104 }],
      }),
    countSnippets: (...args: Parameters<typeof countSnippets>) => countSnippets(...args),
    listSnippetPage: (...args: Parameters<typeof listSnippetPage>) => listSnippetPage(...args),
    searchLibrary: (...args: Parameters<typeof searchLibrary>) => searchLibrary(...args),
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
            ]
          : [],
      ),
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

describe('LibraryPage', () => {
  it('virtualises 50k rows: only a window of rows is ever mounted', async () => {
    const { container } = renderPage();
    await screen.findByText('Snippet 0');
    const rows = container.querySelectorAll('.tv-row');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(100);
    // Only the first page was fetched for the visible window.
    expect(listSnippetPage).toHaveBeenCalledTimes(1);
    expect(listSnippetPage).toHaveBeenCalledWith('all', null, null, 200, 0);
  });

  it('shows rail counts and switches scope from the rail', async () => {
    renderPage();
    const rail = await screen.findByRole('navigation', { name: 'Library' });
    expect(within(rail).getByText('50,000')).toBeDefined();
    fireEvent.click(within(rail).getByRole('button', { name: /Starred/ }));
    expect(countSnippets).toHaveBeenLastCalledWith('starred', null, null);
    fireEvent.click(within(rail).getByRole('button', { name: /Infra/ }));
    expect(countSnippets).toHaveBeenLastCalledWith('folder', 'f-1', null);
  });

  it('narrows by type when a filter chip is picked', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('button', { name: 'Secret' }));
    expect(countSnippets).toHaveBeenLastCalledWith('all', null, 'sensitive');
  });

  it('selecting a row travels the plate and opens a read-only preview', async () => {
    renderPage();
    fireEvent.click(await screen.findByText('Snippet 2'));
    const plate = screen.getByTestId('selection-plate');
    expect(plate.style.transform).toBe('translateY(104px)');

    const preview = screen.getByRole('complementary', { name: 'Snippet preview' });
    expect(within(preview).getByRole('heading', { name: 'Snippet 2' })).toBeDefined();
    expect(within(preview).getByText('body 2')).toBeDefined();
    // A preview, never an edit form (design 1b).
    expect(within(preview).queryAllByRole('textbox')).toHaveLength(0);
    expect(preview.querySelectorAll('input, textarea, select')).toHaveLength(0);
  });

  it('replaces results instantly per keystroke and auto-selects the first hit', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText('Search snippets');
    fireEvent.change(input, { target: { value: 'd' } });
    fireEvent.change(input, { target: { value: 'do' } });
    // One search per keystroke — no debounce.
    expect(searchLibrary).toHaveBeenCalledTimes(2);
    expect(searchLibrary).toHaveBeenLastCalledWith('do', 500);

    await screen.findAllByText('Hit 0');
    expect(screen.queryByText('Snippet 0')).toBeNull();
    // First hit is selected: the plate sits at row 0.
    expect(screen.getByTestId('selection-plate').style.transform).toBe('translateY(0px)');
    // Result count is announced politely.
    expect(screen.getByRole('status').textContent).toBe('3 results');
  });

  it('moves the selection with arrows and opens with Enter', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText('Search snippets');
    fireEvent.change(input, { target: { value: 'docker' } });
    await screen.findAllByText('Hit 0');

    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getByTestId('selection-plate').style.transform).toBe('translateY(52px)');
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(screen.getByTestId('selection-plate').style.transform).toBe('translateY(0px)');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(await screen.findByText('editor-edit-stub')).toBeDefined();
  });

  it('offers two real actions when nothing matches', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    const input = screen.getByLabelText('Search snippets');
    fireEvent.change(input, { target: { value: 'zzz' } });
    await screen.findByText('No snippet matches that');

    // Clear filters returns to browsing.
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect((input as HTMLInputElement).value).toBe('');
    expect(await screen.findByText('Snippet 0')).toBeDefined();

    // Save as snippet goes to the editor with the typed text.
    fireEvent.change(input, { target: { value: 'zzz again' } });
    await screen.findByText('No snippet matches that');
    fireEvent.click(screen.getByRole('button', { name: 'Save as snippet' }));
    expect(await screen.findByText('editor-new-stub')).toBeDefined();
  });

  it('checkbox selection raises the batch bar with a real count', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 0' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 1' }));
    expect(screen.getByText('2 selected')).toBeDefined();
    // Batch mutations arrive with TASK-032/034; the actions are disabled.
    const remove = screen.getByRole('button', { name: 'Delete' });
    expect(remove.hasAttribute('disabled')).toBe(true);

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 1' }));
    expect(screen.getByText('1 selected')).toBeDefined();
  });
});
