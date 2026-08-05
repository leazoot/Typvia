// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { PanelApp } from './panel-app';

const hidePanel = vi.fn(() => Promise.resolve());
const panelReady = vi.fn(() => Promise.resolve());
const panelInsert = vi.fn((id: string) => {
  void id;
  return Promise.resolve();
});
const copySnippet = vi.fn((id: string) => {
  void id;
  return Promise.resolve();
});
const listSnippetPage = vi.fn(() => Promise.resolve<Snippet[]>([]));
const searchLibrary = vi.fn(() => Promise.resolve<Snippet[]>([]));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    hidePanel: () => hidePanel(),
    panelReady: () => panelReady(),
    panelInsert: (id: string) => panelInsert(id),
    copySnippet: (id: string) => copySnippet(id),
    listSnippetPage: (...args: unknown[]) => listSnippetPage(...(args as [])),
    searchLibrary: (...args: unknown[]) => searchLibrary(...(args as [])),
  };
});

// Capture the panel:show subscriber so a test can fire a summon.
let showHandler: ((event: { payload: { destination: string | null } }) => void) | null = null;
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, cb: (e: { payload: { destination: string | null } }) => void) => {
    if (event === 'panel:show') showHandler = cb;
    return Promise.resolve(() => undefined);
  },
}));

function snip(over: Partial<Snippet> = {}): Snippet {
  return {
    id: 's-1',
    title: 'Docker logs',
    body: 'docker logs -f',
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: null,
    trigger: ';dl',
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
    ...over,
  };
}

async function summon(destination: string | null = 'VS Code') {
  await act(async () => {
    showHandler?.({ payload: { destination } });
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  showHandler = null;
  listSnippetPage.mockResolvedValue([]);
  searchLibrary.mockResolvedValue([]);
});

describe('PanelApp', () => {
  it('focuses the search field at frame 0', () => {
    render(<PanelApp />);
    expect(document.activeElement).toBe(screen.getByLabelText('Search snippets'));
  });

  it('hides the panel on Escape', () => {
    render(<PanelApp />);
    fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Escape' });
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  it('loads recent snippets and names the destination on summon', async () => {
    listSnippetPage.mockResolvedValue([snip({ id: 's-1', title: 'Docker logs' })]);
    render(<PanelApp />);
    await summon('VS Code');

    expect(listSnippetPage).toHaveBeenCalledWith('recent', null, null, 20, 0);
    expect(screen.getByText('→ VS Code')).toBeTruthy();
    expect(screen.getByText('Docker logs')).toBeTruthy();
  });

  it('replaces recent with ranked results as you type (no debounce)', async () => {
    render(<PanelApp />);
    await summon();

    searchLibrary.mockResolvedValue([snip({ id: 's-9', title: 'Search hit' })]);
    await act(async () => {
      fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'sea' } });
    });

    expect(searchLibrary).toHaveBeenCalledWith('sea', 20);
    expect(screen.getByText('Search hit')).toBeTruthy();
  });

  it('inserts the selected snippet on Enter', async () => {
    listSnippetPage.mockResolvedValue([snip({ id: 's-1' }), snip({ id: 's-2', title: 'Second' })]);
    render(<PanelApp />);
    await summon();

    fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    expect(panelInsert).toHaveBeenCalledWith('s-1');
  });

  it('moves selection with ArrowDown before inserting', async () => {
    listSnippetPage.mockResolvedValue([snip({ id: 's-1' }), snip({ id: 's-2', title: 'Second' })]);
    render(<PanelApp />);
    await summon();

    const input = screen.getByLabelText('Search snippets');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(panelInsert).toHaveBeenCalledWith('s-2');
  });

  it('copies the selected snippet on Shift+Enter and hides', async () => {
    listSnippetPage.mockResolvedValue([snip({ id: 's-1' })]);
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter', shiftKey: true });
    });
    expect(copySnippet).toHaveBeenCalledWith('s-1');
    expect(hidePanel).toHaveBeenCalled();
  });
});
