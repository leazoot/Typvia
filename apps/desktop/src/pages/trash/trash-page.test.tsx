// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { I18nProvider } from '@typvia/ui';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
  it('is compact: a count, a retention promise, and the rows', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    expect(purgeExpiredTrash).toHaveBeenCalledTimes(1);
    expect(screen.getByText('2 items · kept for 30 days')).toBeDefined();
    expect(screen.getByText('Deleted 2 days ago')).toBeDefined();
    // Remaining life is a number on a hairline, not a dashboard.
    expect(screen.getByText('28 days')).toBeDefined();
    expect(screen.getByText('1 days')).toBeDefined();
    // Nothing destructive is on the surface.
    expect(screen.queryByRole('button', { name: /Delete/ })).toBeNull();
  });

  it('restores a row and says so where the row was', async () => {
    renderTrash();
    const row = await screen.findByRole('button', { name: 'Trashed 1' });
    fireEvent.click(within(row).getByRole('button', { name: 'Restore' }));
    expect(restoreSnippet).toHaveBeenCalledWith('t-1');
    expect(await screen.findByText('Restored to the Library')).toBeDefined();
    // No toast anywhere — the row's own place carries the message.
    await waitFor(() => expect(screen.getByText('1 item · kept for 30 days')).toBeDefined(), {
      timeout: 3000,
    });
  });

  it('keeps permanent deletion behind ··· and confirms in place', async () => {
    renderTrash();
    const row = await screen.findByRole('button', { name: 'Trashed 1' });
    fireEvent.click(within(row).getByRole('button', { name: 'More actions' }));
    // The menu is rendered outside the row on purpose: the row's own surface
    // clips it (anchored-menu.ts).
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete permanently' }));
    expect(deleteSnippetForever).toHaveBeenCalledWith('t-1');
  });

  it('empties the trash only after an in-page confirmation with the real count', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    fireEvent.click(screen.getByRole('button', { name: 'Trash options' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Empty Trash' }));
    const sheet = screen.getByRole('dialog', { name: 'Empty Trash' });
    expect(within(sheet).getByText('Delete 2 items in the trash forever?')).toBeDefined();
    expect(deleteSnippetForever).not.toHaveBeenCalled();

    fireEvent.click(within(sheet).getByRole('button', { name: 'Delete forever' }));
    expect(deleteSnippetForever).toHaveBeenCalledTimes(2);
    expect(await screen.findByText('The trash is clean.')).toBeDefined();
  });

  it('restores a whole selection from the floating strip', async () => {
    renderTrash();
    await screen.findByText('Trashed 1');
    fireEvent.click(screen.getByRole('button', { name: 'Trashed 1' }));
    fireEvent.click(screen.getByRole('button', { name: 'Trashed 2' }));
    const strip = screen.getByRole('group', { name: 'Selection' });
    expect(within(strip).getByText('2 selected')).toBeDefined();
    fireEvent.click(within(strip).getByRole('button', { name: 'Restore 2' }));
    expect(restoreSnippet).toHaveBeenCalledTimes(2);
    expect(await screen.findByText('The trash is clean.')).toBeDefined();
  });

  it('renders a single language — Chinese under the zh locale', async () => {
    render(
      <I18nProvider locale="zh">
        <MemoryRouter initialEntries={['/trash']}>
          <TrashPage />
        </MemoryRouter>
      </I18nProvider>,
    );
    await screen.findByText('Trashed 1');
    expect(screen.getByRole('heading', { name: '回收站' })).toBeDefined();
    expect(screen.queryByText('Trash')).toBeNull();
    expect(screen.getByText('剩 28 天')).toBeDefined();
  });
});
