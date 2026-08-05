// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { TrashPage } from './trash-page';

const DAY = 24 * 60 * 60 * 1000;

function trashed(index: number, deletedAt: number, folderId: string | null): Snippet {
  return {
    id: `t-${String(index)}`,
    title: `Trashed ${String(index)}`,
    body: 'body',
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId,
    trigger: null,
    triggerMode: null,
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: null,
    usageCount: 0,
    version: 1,
    deletedAt,
  };
}

let trashRows: Snippet[] = [];
const purgeExpiredTrash = vi.fn(() => Promise.resolve(0));
const restoreSnippet = vi.fn((id: string) => {
  trashRows = trashRows.filter((row) => row.id !== id);
  return Promise.resolve();
});
const deleteSnippetForever = vi.fn((id: string) => {
  trashRows = trashRows.filter((row) => row.id !== id);
  return Promise.resolve();
});

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    listTrash: () => Promise.resolve(trashRows),
    purgeExpiredTrash: (...args: Parameters<typeof purgeExpiredTrash>) =>
      purgeExpiredTrash(...args),
    restoreSnippet: (...args: Parameters<typeof restoreSnippet>) => restoreSnippet(...args),
    deleteSnippetForever: (...args: Parameters<typeof deleteSnippetForever>) =>
      deleteSnippetForever(...args),
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [{ id: 'f-1', parentId: null, name: 'Infra', sortOrder: 0, createdAt: 1, updatedAt: 1 }]
          : [],
      ),
  };
});

function renderTrash() {
  return render(
    <MemoryRouter initialEntries={['/trash']}>
      <TrashPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  trashRows = [trashed(1, Date.now() - 2 * DAY, 'f-1'), trashed(2, Date.now() - 29 * DAY, null)];
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('TrashPage', () => {
  it('purges expired rows on entry and lists what is left with real days', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    expect(purgeExpiredTrash).toHaveBeenCalledTimes(1);
    // Original folder is shown — restore returns items there.
    expect(screen.getByText('Infra')).toBeDefined();
    expect(screen.getByText('Unsorted')).toBeDefined();
    // 30-day policy with real remaining days per row.
    expect(screen.getByText('28 days left')).toBeDefined();
    expect(screen.getByText('1 days left')).toBeDefined();
    expect(screen.getByText(/After 30 days here/)).toBeDefined();
  });

  it('restores a single row in place', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    fireEvent.click(screen.getAllByRole('button', { name: 'Restore' })[0]!);
    expect(restoreSnippet).toHaveBeenCalledWith('t-1');
    // The listing reloads without the restored row.
    expect(await screen.findByText('1 in trash')).toBeDefined();
  });

  it('forever-deletion demands an explicit confirmation with the real count', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Trashed 1' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Trashed 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete forever' }));
    // Nothing is deleted until the count-bearing confirmation is pressed.
    expect(deleteSnippetForever).not.toHaveBeenCalled();
    expect(screen.getByText(/Delete 2 snippets forever\? This cannot be undone\./)).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Delete 2 snippets' }));
    expect(deleteSnippetForever).toHaveBeenCalledTimes(2);
    expect(await screen.findByText('Trash is empty')).toBeDefined();
  });

  it('restores a whole selection through the batch action', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Trashed 1' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Trashed 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Restore 2 snippets' }));
    expect(restoreSnippet).toHaveBeenCalledTimes(2);
    expect(await screen.findByText('Trash is empty')).toBeDefined();
  });
});
