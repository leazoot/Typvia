// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { I18nProvider } from '@typvia/ui';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { UndoProvider } from '../../workspace/undo';
import { TrashPage } from './trash-page';

const DAY = 24 * 60 * 60 * 1000;

function trashed(
  index: number,
  deletedAt: number,
  { folderId = null, sensitive = false }: { folderId?: string | null; sensitive?: boolean } = {},
): Snippet {
  return {
    id: `t-${String(index)}`,
    title: `Trashed ${String(index)}`,
    body: sensitive ? null : `Body of ${String(index)}\nmore`,
    snippetType: sensitive ? 'sensitive' : 'text',
    securityLevel: sensitive ? 'sensitive' : 'normal',
    description: null,
    folderId,
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
    purgeExpiredTrash: () => purgeExpiredTrash(),
    restoreSnippet: (id: string) => restoreSnippet(id),
    deleteSnippetForever: (id: string) => deleteSnippetForever(id),
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [{ id: 'f-1', parentId: null, name: 'Mail', sortOrder: 0, createdAt: 1, updatedAt: 1 }]
          : [],
      ),
  };
});

function renderTrash(locale: 'en' | 'zh' = 'en') {
  return render(
    <I18nProvider locale={locale}>
      <MemoryRouter initialEntries={['/trash']}>
        <UndoProvider>
          <TrashPage />
        </UndoProvider>
      </MemoryRouter>
    </I18nProvider>,
  );
}

beforeEach(() => {
  trashRows = [
    trashed(1, Date.now() - 2 * DAY, { folderId: 'f-1' }),
    trashed(2, Date.now() - 26 * DAY),
    trashed(3, Date.now() - 9 * DAY, { sensitive: true }),
  ];
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('TrashPage', () => {
  it('says how long each row has left, and warns under a week', async () => {
    const { container } = renderTrash();
    expect(await screen.findByText('Body of 1')).toBeDefined();
    expect(purgeExpiredTrash).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('heading', { name: 'Trash' })).toBeDefined();
    expect(screen.getByText('3', { selector: '.tvt-count' })).toBeDefined();
    expect(screen.getByText('28 days left')).toBeDefined();
    expect(screen.getByText('4 days left')).toBeDefined();
    expect(await screen.findByText(/^Mail · deleted/)).toBeDefined();
    // Only the row with four days left carries the amber dot.
    expect(container.querySelectorAll('.tvt-soon')).toHaveLength(1);
  });

  it('keeps a vault snippet masked in the trash', async () => {
    renderTrash();
    const row = await screen.findByRole('option', { name: /still encrypted/ });
    expect(within(row).getByText('•••••••••••')).toBeDefined();
    expect(within(row).queryByText('Trashed 3')).toBeNull();
  });

  it('restores a row and says where it went', async () => {
    renderTrash();
    await screen.findByText('Body of 1');
    const row = screen.getByRole('option', { name: /Body of 1/ });
    fireEvent.click(within(row).getByRole('button', { name: 'Restore' }));
    expect(restoreSnippet).toHaveBeenCalledWith('t-1');
    expect(await screen.findByText('/t1 is back.')).toBeDefined();
    expect(screen.getByText('It is in “Mail” again.')).toBeDefined();
  });

  it('deletes one for good from its right-click menu without asking', async () => {
    renderTrash();
    await screen.findByText('Body of 2');
    fireEvent.contextMenu(screen.getByRole('option', { name: /Body of 2/ }), {
      clientX: 20,
      clientY: 20,
    });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete for good' }));
    expect(deleteSnippetForever).toHaveBeenCalledWith('t-2');
    expect(await screen.findByText('/t2 is gone for good.')).toBeDefined();
  });

  it('asks once, with the real count, before emptying the trash', async () => {
    renderTrash();
    await screen.findByText('Body of 1');
    fireEvent.click(screen.getByRole('button', { name: 'Empty the trash' }));
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('Delete the 3 in the trash for good?')).toBeDefined();
    expect(deleteSnippetForever).not.toHaveBeenCalled();
    // Focus starts on the answer that keeps everything.
    expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'Not now' }));

    fireEvent.click(within(dialog).getByRole('button', { name: 'Delete 3 for good' }));
    expect(deleteSnippetForever).toHaveBeenCalledTimes(3);
  });

  it('shows the empty trash with its promise when nothing is in it', async () => {
    trashRows = [];
    renderTrash();
    expect(await screen.findByRole('heading', { name: 'The trash is empty.' })).toBeDefined();
    expect(screen.getByText(/wait here for 30 days/)).toBeDefined();
  });

  it('renders a single language — Chinese under the zh locale', async () => {
    renderTrash('zh');
    await screen.findByText('Body of 1');
    expect(screen.getByRole('heading', { name: '回收站' })).toBeDefined();
    expect(screen.getByText('还剩 28 天')).toBeDefined();
    expect(screen.getByText('3 条', { selector: '.tvt-count' })).toBeDefined();
    expect(screen.queryByText('Trash')).toBeNull();
  });
});
