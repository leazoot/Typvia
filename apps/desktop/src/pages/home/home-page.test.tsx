// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
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
  };
});

function renderHome() {
  return render(
    <MemoryRouter initialEntries={['/']}>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/library" element={<div>library-stub</div>} />
        <Route path="/editor" element={<div>editor-stub</div>} />
        <Route path="/editor/:id" element={<div>editor-edit-stub</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  total = 12481;
  recentRows = [];
});

describe('HomePage', () => {
  it('shows a chronological ledger with time separators and per-row use counts', async () => {
    recentRows = [
      usedSnippet(1, NOW - 60_000, 4),
      usedSnippet(2, NOW - 65_000, 2),
      usedSnippet(3, NOW - 26 * 60 * 60 * 1000, 6),
    ];
    const { container } = renderHome();
    await screen.findByText('Used 1');
    // Day separators, chronological — never a ranking.
    expect(screen.getByText(/Reached for today/)).toBeDefined();
    expect(screen.getByText(/Yesterday|Earlier/)).toBeDefined();
    expect(screen.getByText('4×')).toBeDefined();
    // Rows fade in top-down 40ms apart.
    const rows = container.querySelectorAll('.tv-home-row');
    expect(rows.length).toBe(3);
    expect((rows[0] as HTMLElement).style.animationDelay).toBe('0ms');
    expect((rows[1] as HTMLElement).style.animationDelay).toBe('40ms');
    expect((rows[2] as HTMLElement).style.animationDelay).toBe('80ms');
  });

  it('shows the first-run state when the library is empty', async () => {
    total = 0;
    renderHome();
    expect(await screen.findByText('Save your first snippet')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'New snippet' }));
    expect(await screen.findByText('editor-stub')).toBeDefined();
  });

  it('hands the search line over to the Library on first keystroke', async () => {
    renderHome();
    await screen.findByText(/12,481 snippets/);
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'd' } });
    expect(await screen.findByText('library-stub')).toBeDefined();
  });

  it('shows local counts and not-configured channels in the status rail', async () => {
    recentRows = [usedSnippet(1, NOW - 1000, 1)];
    renderHome();
    const rail = await screen.findByRole('complementary', { name: 'Status' });
    const snippetsRow = within(rail).getByText('Snippets').parentElement;
    expect(within(snippetsRow as HTMLElement).getByText('12,481')).toBeDefined();
    const trashRow = within(rail).getByText('Trash').parentElement;
    expect(within(trashRow as HTMLElement).getByText('3')).toBeDefined();
    expect(within(rail).getAllByText('not configured')).toHaveLength(6);
    expect(within(rail).getByText(/Nothing leaves this Mac/)).toBeDefined();
  });
});
