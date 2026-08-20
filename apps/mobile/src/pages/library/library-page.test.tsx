// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LibraryPage } from './library-page';

const ipc = vi.hoisted(() => ({
  countSnippets: vi.fn(),
  listSnippetPage: vi.fn(),
  searchLibrary: vi.fn(),
  listFolderChildren: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

function snippet(id: string, title: string, overrides: Partial<Snippet> = {}): Snippet {
  return {
    id,
    title,
    body: 'docker logs -f --tail 200 nginx',
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: null,
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
    deletedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  ipc.countSnippets.mockResolvedValue(2);
  ipc.listSnippetPage.mockResolvedValue([
    snippet('s-1', 'Docker tail logs'),
    snippet('s-2', 'Docker compose up'),
  ]);
  ipc.searchLibrary.mockResolvedValue([]);
  ipc.listFolderChildren.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile LibraryPage', () => {
  it('browses the paged list and announces the total politely', async () => {
    render(<LibraryPage onOpen={() => {}} />);

    expect(await screen.findByText('Docker tail logs')).toBeDefined();
    expect(ipc.listSnippetPage).toHaveBeenCalledWith('all', null, null, 100, 0);
    const count = screen.getByRole('status');
    expect(count.getAttribute('aria-live')).toBe('polite');
    expect(count.textContent).toBe('2');
  });

  it('groups browse rows under their folder and files the rest as Unfiled', async () => {
    ipc.listFolderChildren.mockResolvedValue([
      {
        id: 'f-1',
        parentId: null,
        name: 'Work',
        sortOrder: 0,
        createdAt: 1,
        updatedAt: 1,
      },
    ]);
    ipc.listSnippetPage.mockResolvedValue([
      snippet('s-1', 'Docker tail logs', { folderId: 'f-1' }),
      snippet('s-2', 'Docker compose up'),
    ]);
    render(<LibraryPage onOpen={() => {}} />);

    expect(await screen.findByText('Work')).toBeDefined();
    expect(screen.getByText('Unfiled')).toBeDefined();
  });

  it('shows the usage count as the trailing glyph, only when used', async () => {
    ipc.listSnippetPage.mockResolvedValue([
      snippet('s-1', 'Docker tail logs', { usageCount: 24 }),
      snippet('s-2', 'Docker compose up'),
    ]);
    render(<LibraryPage onOpen={() => {}} />);

    expect(await screen.findByText('24×')).toBeDefined();
    expect(screen.queryByText('0×')).toBeNull();
  });

  it('opens the detail screen from a row tap', async () => {
    const onOpen = vi.fn();
    render(<LibraryPage onOpen={onOpen} />);

    fireEvent.click(await screen.findByText('Docker tail logs'));
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen.mock.calls[0]?.[0]?.id).toBe('s-1');
  });

  it('searches instantly on every keystroke and replaces the results', async () => {
    ipc.searchLibrary.mockResolvedValueOnce([snippet('s-1', 'Docker tail logs')]);
    ipc.searchLibrary.mockResolvedValueOnce([snippet('s-2', 'Docker compose up')]);
    render(<LibraryPage onOpen={() => {}} />);
    await screen.findByText('Docker tail logs');

    const field = screen.getByLabelText('Search library');
    fireEvent.change(field, { target: { value: 'd' } });
    expect(ipc.searchLibrary).toHaveBeenCalledWith('d', 200);

    fireEvent.change(field, { target: { value: 'do' } });
    expect(ipc.searchLibrary).toHaveBeenCalledWith('do', 200);
    expect(await screen.findByText('Docker compose up')).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByText('Docker tail logs')).toBeNull();
    });
    expect(ipc.searchLibrary).toHaveBeenCalledTimes(2);
  });

  it('filters by type through the paged IPC, never client-side', async () => {
    render(<LibraryPage onOpen={() => {}} />);
    await screen.findByText('Docker tail logs');

    fireEvent.click(screen.getByRole('button', { name: 'Code' }));
    await waitFor(() => {
      expect(ipc.listSnippetPage).toHaveBeenCalledWith('all', null, 'code', 100, 0);
    });
    expect(ipc.countSnippets).toHaveBeenCalledWith('all', null, 'code');
    expect(screen.getByRole('button', { name: 'Code' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('disables the type tabs while a query is active (searchLibrary has no type filter)', async () => {
    ipc.searchLibrary.mockResolvedValue([snippet('s-1', 'Docker tail logs')]);
    render(<LibraryPage onOpen={() => {}} />);
    await screen.findByText('Docker tail logs');

    fireEvent.change(screen.getByLabelText('Search library'), { target: { value: 'docker' } });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Code' }).getAttribute('aria-disabled')).toBe(
        'true',
      );
    });
    fireEvent.click(screen.getByRole('button', { name: 'Code' }));
    // The browse list was fetched once, unfiltered — the tap changed nothing.
    expect(ipc.listSnippetPage).toHaveBeenCalledTimes(1);
    expect(ipc.listSnippetPage).toHaveBeenCalledWith('all', null, null, 100, 0);
  });

  it('names the query in the no-results state and can clear it', async () => {
    ipc.searchLibrary.mockResolvedValue([]);
    render(<LibraryPage onOpen={() => {}} />);
    await screen.findByText('Docker tail logs');

    fireEvent.change(screen.getByLabelText('Search library'), { target: { value: 'zzz' } });
    expect(await screen.findByText('No snippet matches “zzz”')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));
    expect(await screen.findByText('Docker tail logs')).toBeDefined();
  });

  it('shows the empty-library state honestly', async () => {
    ipc.countSnippets.mockResolvedValue(0);
    ipc.listSnippetPage.mockResolvedValue([]);
    render(<LibraryPage onOpen={() => {}} />);

    expect(await screen.findByText('Your library is empty')).toBeDefined();
    expect(screen.getByText(/Save a snippet with the centre button/)).toBeDefined();
  });

  it('says the data is safe first when the list fails, and can retry', async () => {
    ipc.listSnippetPage.mockRejectedValueOnce(new Error('io'));
    render(<LibraryPage onOpen={() => {}} />);

    expect(await screen.findByText('Your snippets are safe on this device.')).toBeDefined();
    ipc.listSnippetPage.mockResolvedValueOnce([snippet('s-1', 'Docker tail logs')]);
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('Docker tail logs')).toBeDefined();
  });

  it('loads the next page on demand when more rows exist', async () => {
    ipc.countSnippets.mockResolvedValue(150);
    render(<LibraryPage onOpen={() => {}} />);
    await screen.findByText('Docker tail logs');

    ipc.listSnippetPage.mockResolvedValueOnce([snippet('s-3', 'Prune docker system')]);
    fireEvent.click(screen.getByRole('button', { name: /Load more/ }));
    expect(await screen.findByText('Prune docker system')).toBeDefined();
    expect(ipc.listSnippetPage).toHaveBeenLastCalledWith('all', null, null, 100, 2);
  });
});
