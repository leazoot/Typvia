// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
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
    <MemoryRouter>
      <LibraryPage />
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
