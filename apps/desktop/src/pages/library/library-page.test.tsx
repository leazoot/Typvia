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
const batchMoveSnippets = vi.fn(() => Promise.resolve());
const batchTagSnippets = vi.fn(() => Promise.resolve());
const batchTrashSnippets = vi.fn(() => Promise.resolve());
const createFolder = vi.fn(() => Promise.resolve({}));
const updateFolder = vi.fn(() => Promise.resolve({}));
const deleteFolder = vi.fn(() => Promise.resolve());
const createTag = vi.fn(() => Promise.resolve({}));
const renameTag = vi.fn(() => Promise.resolve());
const deleteTag = vi.fn(() => Promise.resolve());
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
        trash: 3,
        folders: [
          { folderId: 'f-1', count: 2104 },
          { folderId: 'f-2', count: 12 },
        ],
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
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 1' }));
    expect(screen.getByText('1 selected')).toBeDefined();
  });

  it('batch move and tag go through the batch APIs via popovers', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 0' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 1' }));

    fireEvent.click(screen.getByRole('button', { name: 'Move to folder' }));
    const movePopover = screen.getByRole('listbox', { name: 'Move to folder' });
    fireEvent.click(within(movePopover).getByRole('button', { name: 'Infra' }));
    expect(batchMoveSnippets).toHaveBeenCalledWith(['s-0', 's-1'], 'f-1');
    // The batch clears once the operation lands.
    expect(await screen.findByText(/Local library/)).toBeDefined();
    expect(screen.queryByText('2 selected')).toBeNull();

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 0' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add tag' }));
    const tagPopover = screen.getByRole('listbox', { name: 'Add tag' });
    fireEvent.click(within(tagPopover).getByRole('button', { name: 'prod' }));
    expect(batchTagSnippets).toHaveBeenCalledWith(['s-0'], 'tag-1');
  });

  it('batch delete asks with the real number before trashing', async () => {
    renderPage();
    await screen.findByText('Snippet 0');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 0' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Snippet 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    // Real count, never "Clear all".
    const confirm = screen.getByRole('button', { name: 'Delete 2 snippets' });
    fireEvent.click(confirm);
    expect(batchTrashSnippets).toHaveBeenCalledWith(['s-0', 's-2']);
  });

  it('creates, renames and deletes folders with inline confirmation', async () => {
    renderPage();
    const rail = await screen.findByRole('navigation', { name: 'Library' });

    // Create.
    fireEvent.click(within(rail).getAllByRole('button', { name: 'New' })[0]!);
    const input = within(rail).getByLabelText('New folder name');
    fireEvent.change(input, { target: { value: 'Notes' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(createFolder).toHaveBeenCalledWith({ name: 'Notes', parentId: null, sortOrder: 2 });

    // Activate a folder to reveal its management actions, then rename.
    fireEvent.click(within(rail).getByRole('button', { name: /Infra/ }));
    fireEvent.click(within(rail).getByRole('button', { name: 'Rename' }));
    const rename = within(rail).getByLabelText('Rename Infra');
    fireEvent.change(rename, { target: { value: 'Infrastructure' } });
    fireEvent.keyDown(rename, { key: 'Enter' });
    expect(updateFolder).toHaveBeenCalledWith({
      id: 'f-1',
      name: 'Infrastructure',
      parentId: null,
      sortOrder: 0,
    });

    // Delete: inline confirmation carries the real subtree count.
    fireEvent.click(within(rail).getByRole('button', { name: /Infra/ }));
    fireEvent.click(within(rail).getByRole('button', { name: 'Delete' }));
    expect(within(rail).getByText(/2,104 snippets move out/)).toBeDefined();
    const confirmRow = within(rail).getByText(/snippets move out/).parentElement;
    fireEvent.click(within(confirmRow as HTMLElement).getByRole('button', { name: 'Delete' }));
    expect(deleteFolder).toHaveBeenCalledWith('f-1');
  });

  it('reorders sibling folders with alt+arrows', async () => {
    renderPage();
    const rail = await screen.findByRole('navigation', { name: 'Library' });
    fireEvent.keyDown(within(rail).getByRole('button', { name: /Infra/ }), {
      key: 'ArrowDown',
      altKey: true,
    });
    // Swap persists a clean sequential order: both siblings get new slots.
    expect(updateFolder).toHaveBeenCalledWith({
      id: 'f-2',
      name: 'Support',
      parentId: null,
      sortOrder: 0,
    });
    expect(updateFolder).toHaveBeenCalledWith({
      id: 'f-1',
      name: 'Infra',
      parentId: null,
      sortOrder: 1,
    });
  });

  it('creates and deletes tags from the rail', async () => {
    renderPage();
    const rail = await screen.findByRole('navigation', { name: 'Library' });
    fireEvent.click(within(rail).getAllByRole('button', { name: 'New' })[1]!);
    const input = within(rail).getByLabelText('New tag name');
    fireEvent.change(input, { target: { value: 'urgent' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(createTag).toHaveBeenCalledWith('urgent');

    fireEvent.click(within(rail).getByRole('button', { name: 'prod' }));
    fireEvent.click(within(rail).getByRole('button', { name: 'Delete' }));
    expect(within(rail).getByText(/Delete tag “prod”/)).toBeDefined();
    const confirmRow = within(rail).getByText(/Delete tag/).parentElement;
    fireEvent.click(within(confirmRow as HTMLElement).getByRole('button', { name: 'Delete' }));
    expect(deleteTag).toHaveBeenCalledWith('tag-1');
  });
});
