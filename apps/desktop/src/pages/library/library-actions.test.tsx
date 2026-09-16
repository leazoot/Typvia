// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Folder, Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { UndoProvider } from '../../workspace/undo';
import { LibraryPage } from './library-page';

function snippet(index: number, folderId: string | null, body?: string): Snippet {
  return {
    id: `s-${String(index)}`,
    title: `Title ${String(index)}`,
    body: body ?? `Body line ${String(index)}`,
    snippetType: 'text',
    securityLevel: 'normal',
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
    deletedAt: null,
  };
}

function folder(id: string, name: string, sortOrder: number): Folder {
  return { id, parentId: null, name, sortOrder, createdAt: 1, updatedAt: 1 };
}

const ROWS = [snippet(0, 'f-1'), snippet(1, null, 'Dear {{name}}'), snippet(2, 'f-2')];

const ipc = vi.hoisted(() => ({
  trashSnippet: vi.fn(),
  restoreSnippet: vi.fn(),
  batchMoveSnippets: vi.fn(),
  createFolder: vi.fn(),
  updateFolder: vi.fn(),
  deleteFolder: vi.fn(),
  mergeFolders: vi.fn(),
  reorderFolders: vi.fn(),
  mainInsert: vi.fn(),
  mainInsertTemplate: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    ...ipc,
    libraryCounts: () =>
      Promise.resolve({
        total: 3,
        recent: 0,
        starred: 0,
        unsorted: 1,
        trash: 0,
        folders: [
          { folderId: 'f-1', count: 1 },
          { folderId: 'f-2', count: 1 },
        ],
      }),
    countSnippets: () => Promise.resolve(ROWS.length),
    listSnippetPage: (view: string, folderId: string | null) =>
      Promise.resolve(view === 'folder' ? ROWS.filter((row) => row.folderId === folderId) : ROWS),
    searchLibraryDeep: () => Promise.resolve([]),
    searchSnippets: () => Promise.resolve([]),
    vaultList: () => Promise.resolve([]),
    syncStatus: () => Promise.reject(new Error('no host in tests')),
    templateVariables: (body: string) => Promise.resolve(body.includes('{{name}}') ? ['name'] : []),
    templateFields: () =>
      Promise.resolve([
        {
          id: 'tf-1',
          name: 'name',
          label: 'Recipient',
          fieldType: 'single_line_text',
          defaultValue: null,
          options: [],
          validation: null,
          isRequired: true,
          sortOrder: 0,
          platformOverrides: null,
        },
      ]),
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null ? [folder('f-1', 'Infra', 0), folder('f-2', 'Mail', 1)] : [],
      ),
  };
});

function renderPage() {
  return render(
    <I18nProvider locale="en">
      <MemoryRouter initialEntries={['/']}>
        <UndoProvider>
          <Routes>
            <Route path="/" element={<LibraryPage />} />
            <Route path="/editor/:id" element={<div>editor-stub</div>} />
          </Routes>
        </UndoProvider>
      </MemoryRouter>
    </I18nProvider>,
  );
}

beforeEach(() => {
  for (const mock of Object.values(ipc)) mock.mockResolvedValue(undefined);
  ipc.createFolder.mockResolvedValue(folder('f-new', 'Infra', 0));
  ipc.mergeFolders.mockResolvedValue(['s-0']);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  Reflect.deleteProperty(document, 'elementFromPoint');
});

async function rowMenuFor(text: string) {
  const row = (await screen.findByText(text, { selector: '.tvl-row-body' })).closest(
    '[role="option"]',
  );
  if (row === null) throw new Error(`no row for ${text}`);
  fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
  return screen.getByRole('menu', { name: 'Snippet actions' });
}

describe('LibraryPage — a snippet’s right-click menu', () => {
  it('deletes without asking and offers it back on a note', async () => {
    renderPage();
    const menu = await rowMenuFor('Body line 0');
    fireEvent.click(within(menu).getByRole('menuitem', { name: /Delete snippet/ }));
    await waitFor(() => expect(ipc.trashSnippet).toHaveBeenCalledWith('s-0'));

    const note = await screen.findByRole('status', { name: 'Deleted /t0.' });
    fireEvent.click(within(note).getByRole('button', { name: 'Undo ⌘Z' }));
    await waitFor(() => expect(ipc.restoreSnippet).toHaveBeenCalledWith('s-0'));
  });

  it('asks for a variable snippet’s values in the detail pane before inserting it', async () => {
    renderPage();
    await screen.findByText('Body line 0', { selector: '.tvl-row-body' });
    // Variables are read per row; wait until the row knows it has one.
    await screen.findByText('1 variable');
    const menu = await rowMenuFor('Dear {{name}}');
    fireEvent.click(within(menu).getByRole('menuitem', { name: /Fill variables, then insert/ }));
    const field = await screen.findByLabelText(/Recipient/);
    fireEvent.change(field, { target: { value: 'Ada' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() =>
      expect(ipc.mainInsertTemplate).toHaveBeenCalledWith('s-1', { name: 'Ada' }, 'paste'),
    );
    expect(ipc.mainInsert).not.toHaveBeenCalled();
  });

  it('moves a snippet through the collection submenu, and not to where it already is', async () => {
    renderPage();
    const menu = await rowMenuFor('Body line 0');
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Move to collection' }));
    const infra = screen.getByRole('menuitem', { name: /Infra/ });
    expect(infra.getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(screen.getByRole('menuitem', { name: 'Unsorted' }));
    await waitFor(() => expect(ipc.batchMoveSnippets).toHaveBeenCalledWith(['s-0'], null));
  });

  it('deletes the picked snippet with ⌘⌫ and takes it back with ⌘Z', async () => {
    renderPage();
    await screen.findByText('Body line 0', { selector: '.tvl-row-body' });
    const list = screen.getByRole('listbox', { name: 'All snippets' });
    fireEvent.keyDown(list, { key: 'Backspace', metaKey: true });
    await waitFor(() => expect(ipc.trashSnippet).toHaveBeenCalledWith('s-0'));
    await screen.findByRole('status', { name: 'Deleted /t0.' });
    fireEvent.keyDown(list, { key: 'z', metaKey: true });
    await waitFor(() => expect(ipc.restoreSnippet).toHaveBeenCalledWith('s-0'));
  });

  it('drops a dragged row onto a collection to move it there', async () => {
    renderPage();
    const row = (await screen.findByText('Body line 0', { selector: '.tvl-row-body' })).closest(
      '[role="option"]',
    );
    if (row === null) throw new Error('no row');
    const mail = screen.getByRole('radio', { name: /Mail/ });
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: () => mail,
    });
    fireEvent.pointerDown(row, { button: 0, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(document, { clientX: 30, clientY: 30 });
    expect(await screen.findByText('Drop here · +1')).toBeDefined();
    fireEvent.pointerUp(document);
    await waitFor(() => expect(ipc.batchMoveSnippets).toHaveBeenCalledWith(['s-0'], 'f-2'));
  });
});

describe('LibraryPage — arranging collections', () => {
  async function manage() {
    renderPage();
    await screen.findByText('Body line 0', { selector: '.tvl-row-body' });
    fireEvent.click(screen.getByRole('button', { name: 'Manage' }));
    return screen.getByText('Collections · arranging');
  }

  it('renames a collection in place on one line', async () => {
    await manage();
    fireEvent.click(screen.getByRole('button', { name: 'Rename Infra' }));
    const line = screen.getByLabelText('New name for Infra');
    fireEvent.change(line, { target: { value: 'Ops' } });
    fireEvent.keyDown(line, { key: 'Enter' });
    await waitFor(() =>
      expect(ipc.updateFolder).toHaveBeenCalledWith({
        id: 'f-1',
        name: 'Ops',
        parentId: null,
        sortOrder: 0,
      }),
    );
    expect(screen.getByText('Built in · cannot be deleted')).toBeDefined();
  });

  it('makes a new collection from the line under the list', async () => {
    await manage();
    fireEvent.click(screen.getByRole('button', { name: /New collection/ }));
    const line = screen.getByLabelText('Name of the new collection');
    fireEvent.change(line, { target: { value: 'Replies' } });
    fireEvent.keyDown(line, { key: 'Enter' });
    await waitFor(() =>
      expect(ipc.createFolder).toHaveBeenCalledWith({
        name: 'Replies',
        parentId: null,
        sortOrder: 2,
      }),
    );
  });

  it('deletes a collection at once and brings it back with its snippets', async () => {
    await manage();
    fireEvent.click(screen.getByRole('button', { name: 'Delete Infra' }));
    await waitFor(() => expect(ipc.deleteFolder).toHaveBeenCalledWith('f-1'));
    const note = await screen.findByRole('status', { name: 'Deleted the “Infra” collection.' });
    fireEvent.click(within(note).getByRole('button', { name: 'Undo ⌘Z' }));
    await waitFor(() =>
      expect(ipc.createFolder).toHaveBeenCalledWith({
        name: 'Infra',
        parentId: null,
        sortOrder: 0,
      }),
    );
    await waitFor(() => expect(ipc.batchMoveSnippets).toHaveBeenCalledWith(['s-0'], 'f-new'));
  });

  it('asks once on a note before merging one collection into another', async () => {
    await manage();
    const mailRow = screen.getByRole('button', { name: 'Rename Mail' }).closest('.tvl-manage-row');
    const infraHandle = screen
      .getByRole('button', { name: 'Rename Infra' })
      .closest('.tvl-manage-row')
      ?.querySelector('.tvl-handle');
    if (mailRow === null || infraHandle === null || infraHandle === undefined) {
      throw new Error('rows missing');
    }
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: () => mailRow,
    });
    fireEvent.pointerDown(infraHandle, { button: 0, clientX: 0, clientY: 0 });
    // A zero-height row in jsdom puts y = 0.5 in the middle zone: "onto", not "between".
    fireEvent.pointerMove(document, { clientX: 20, clientY: 0.5 });
    fireEvent.pointerUp(document);

    const note = await screen.findByRole('status', { name: 'Merge “Infra” into “Mail”?' });
    expect(ipc.mergeFolders).not.toHaveBeenCalled();
    fireEvent.click(within(note).getByRole('button', { name: 'Merge' }));
    await waitFor(() => expect(ipc.mergeFolders).toHaveBeenCalledWith('f-1', 'f-2'));
    expect(
      await screen.findByRole('status', { name: 'Merged “Infra” into “Mail”.' }),
    ).toBeDefined();
  });
});
