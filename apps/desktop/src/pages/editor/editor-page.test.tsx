// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { IpcError } from '@typvia/shared';
import type { Snippet, TemplateField } from '@typvia/shared';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useParams } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { VaultProvider } from '../../vault/vault-context';
import { EditorPage } from './editor-page';
import { SAVE_DEBOUNCE_MS } from './use-editor-draft';

const EXISTING: Snippet = {
  id: 's-1',
  title: 'Existing snippet',
  body: 'Hello {{name}}, welcome.',
  snippetType: 'text',
  securityLevel: 'normal',
  description: '既有片段',
  folderId: null,
  trigger: ';hi',
  triggerMode: 'delimiter',
  language: null,
  isFavorite: false,
  isPinned: false,
  isEnabled: true,
  createdAt: 1,
  updatedAt: 1,
  lastUsedAt: null,
  usageCount: 0,
  version: 3,
  deletedAt: null,
};

const NAME_FIELD: TemplateField = {
  id: 'tf-1',
  name: 'name',
  label: 'name',
  fieldType: 'single_line_text',
  defaultValue: 'Ada',
  options: [],
  validation: null,
  isRequired: false,
  sortOrder: 0,
  platformOverrides: null,
};

const createSnippet = vi.fn();
const updateSnippet = vi.fn();
const detectSensitive = vi.fn();
const convertToSensitive = vi.fn();
const vaultStatusMock = vi.fn();
const templatePreview = vi.fn();

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    getSnippet: (id: string) =>
      id === 's-1'
        ? Promise.resolve(EXISTING)
        : id === 's-sec'
          ? Promise.resolve({
              ...EXISTING,
              id: 's-sec',
              title: 'APIKEY',
              body: null,
              snippetType: 'sensitive',
              securityLevel: 'sensitive',
            })
          : Promise.reject(new actual.IpcError('not_found', 'not found')),
    createSnippet: (...args: unknown[]) => createSnippet(...args) as Promise<Snippet>,
    updateSnippet: (...args: unknown[]) => updateSnippet(...args) as Promise<Snippet>,
    detectSensitive: (...args: unknown[]) => detectSensitive(...args) as Promise<string[]>,
    snippetConvertToSensitive: (...args: unknown[]) =>
      convertToSensitive(...args) as Promise<Snippet>,
    vaultStatus: () => vaultStatusMock() as Promise<SharedModule.VaultStatus>,
    templateFields: () => Promise.resolve([NAME_FIELD]),
    templatePreview: (...args: unknown[]) => templatePreview(...args) as Promise<string>,
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [{ id: 'f-1', parentId: null, name: 'Infra', sortOrder: 0, createdAt: 1, updatedAt: 1 }]
          : [],
      ),
  };
});

function HistoryLanding() {
  const { id } = useParams();
  return <div>History of {id}</div>;
}

function renderAt(path: string, state?: unknown) {
  return render(
    <MemoryRouter initialEntries={[{ pathname: path, state }]}>
      <VaultProvider>
        <Routes>
          <Route path="/editor" element={<EditorPage />} />
          <Route path="/editor/:id" element={<EditorPage />} />
          <Route path="/editor/:id/history" element={<HistoryLanding />} />
          <Route path="/vault" element={<div>Vault landing</div>} />
        </Routes>
      </VaultProvider>
    </MemoryRouter>,
  );
}

async function settleAutosave() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS + 100);
  });
}

/** Flushes pending microtasks (initial load) under fake timers. */
async function flush() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  createSnippet.mockImplementation((input: { title: string; body: string }) =>
    Promise.resolve({
      ...EXISTING,
      id: 's-new',
      title: input.title,
      body: input.body,
      version: 1,
      updatedAt: Date.now(),
    }),
  );
  updateSnippet.mockImplementation((input: { id: string; title: string; body: string }) =>
    Promise.resolve({ ...EXISTING, ...input, version: 4, updatedAt: Date.now() }),
  );
  detectSensitive.mockResolvedValue([]);
  templatePreview.mockImplementation(
    (body: string, _fields: unknown, values: Record<string, string>) =>
      Promise.resolve(body.replace('{{name}}', values.name ?? '')),
  );
  vaultStatusMock.mockResolvedValue({
    initialized: true,
    unlocked: false,
    unlockedAt: null,
    lastActivityAt: null,
    idleTimeoutMs: 300_000,
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe('EditorPage — writing without a save button', () => {
  it('auto-saves a new draft once a title exists, then says it is saved', async () => {
    renderAt('/editor');
    fireEvent.change(screen.getByLabelText('Snippet title'), {
      target: { value: 'Deploy checklist' },
    });
    fireEvent.change(screen.getByLabelText('Snippet body'), {
      target: { value: 'kubectl get pods' },
    });
    await settleAutosave();

    expect(createSnippet).toHaveBeenCalledTimes(1);
    expect(createSnippet.mock.calls[0]?.[0]).toMatchObject({
      title: 'Deploy checklist',
      body: 'kubectl get pods',
    });
    expect(screen.getByRole('status').textContent).toContain('Saved · v1');
  });

  it('keeps the caret in the body field across a quiet auto-save', async () => {
    renderAt('/editor');
    fireEvent.change(screen.getByLabelText('Snippet title'), {
      target: { value: 'Deploy checklist' },
    });
    const body = screen.getByLabelText('Snippet body');
    body.focus();
    fireEvent.change(body, { target: { value: 'kubectl get pods' } });
    await settleAutosave();

    expect(createSnippet).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(screen.getByLabelText('Snippet body'));
    fireEvent.change(screen.getByLabelText('Snippet body'), {
      target: { value: 'kubectl get pods -A' },
    });
    await settleAutosave();
    expect(document.activeElement).toBe(screen.getByLabelText('Snippet body'));
  });

  it('does not try to save while the title is blank', async () => {
    renderAt('/editor');
    fireEvent.change(screen.getByLabelText('Snippet body'), { target: { value: 'body only' } });
    await settleAutosave();
    expect(createSnippet).not.toHaveBeenCalled();
    expect(screen.getByRole('status').textContent).toBe('Draft');
  });

  it('auto-saves edits to an existing snippet and reflects the bumped version', async () => {
    renderAt('/editor/s-1');
    await flush();
    const title = screen.getByLabelText<HTMLInputElement>('Snippet title');
    expect(title.value).toBe('Existing snippet');

    fireEvent.change(title, { target: { value: 'Existing snippet, renamed' } });
    await settleAutosave();

    expect(updateSnippet).toHaveBeenCalledTimes(1);
    expect(updateSnippet.mock.calls[0]?.[0]).toMatchObject({
      id: 's-1',
      title: 'Existing snippet, renamed',
    });
    expect(screen.getByRole('status').textContent).toContain('Saved · v4');
  });

  it('starts a new snippet from words handed over by the clipboard', async () => {
    renderAt('/editor', { draftBody: 'Pasted words' });
    expect(screen.getByRole('button', { name: 'Edit snippet body' }).textContent).toBe(
      'Pasted words',
    );
  });

  it('puts the caret in the trigger when asked to change the trigger', async () => {
    renderAt('/editor/s-1', { focus: 'trigger' });
    await flush();
    expect(document.activeElement).toBe(screen.getByLabelText('Trigger'));
  });
});

describe('EditorPage — variables and the specimen', () => {
  it('washes {{variables}} in the body and describes each one', async () => {
    const { container } = renderAt('/editor/s-1');
    await flush();
    const tokens = container.querySelectorAll('.tve-token');
    expect(tokens[0]?.textContent).toBe('{{name}}');
    expect(
      screen.getByText('1 variables. Tab through them when inserting; any left empty stay empty.'),
    ).toBeDefined();
    expect(screen.getByText('Defaults to “Ada”')).toBeDefined();
    expect(screen.getByText('One line of text')).toBeDefined();
  });

  it('renders the specimen through the template engine and switches value modes', async () => {
    renderAt('/editor/s-1');
    await flush();
    await flush();
    const specimen = screen.getByRole('complementary', { name: 'What it inserts' });
    expect(specimen.textContent).toContain('Hello Ada, welcome.');
    expect(specimen.textContent).toContain('All 1 variables have defaults');

    fireEvent.click(screen.getByRole('radio', { name: 'Left blank' }));
    await flush();
    expect(templatePreview).toHaveBeenLastCalledWith('Hello {{name}}, welcome.', [NAME_FIELD], {
      name: '',
    });
    expect(screen.getByRole('complementary', { name: 'What it inserts' }).textContent).toContain(
      'Hello , welcome.',
    );
  });

  it('keeps each tool beside what it acts on instead of in a group at the bottom', async () => {
    renderAt('/editor/s-1');
    await flush();
    await flush();
    const specimen = screen.getByRole('complementary', { name: 'What it inserts' });
    expect(within(specimen).getByRole('button', { name: 'Test insert' })).toBeDefined();
    expect(within(specimen).queryByRole('button', { name: 'Organize' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Organize' })).toBeDefined();
    expect(screen.queryByText('Tools')).toBeNull();

    fireEvent.click(within(specimen).getByRole('button', { name: 'Version history' }));
    await flush();
    expect(screen.getByText('History of s-1')).toBeDefined();
  });

  it('offers no version history before the first save', async () => {
    renderAt('/editor');
    await flush();
    const specimen = screen.getByRole('complementary', { name: 'What it inserts' });
    expect(within(specimen).queryByRole('button', { name: 'Version history' })).toBeNull();
  });
});

describe('EditorPage — the rest of a snippet', () => {
  it('shows the sensitive hint as advice while the save still succeeds', async () => {
    detectSensitive.mockResolvedValue(['jwt', 'api_key']);
    renderAt('/editor/s-1');
    await flush();
    fireEvent.change(screen.getByLabelText('Snippet title'), {
      target: { value: 'Now with a token' },
    });
    await settleAutosave();

    expect(screen.getByRole('status').textContent).toContain('Saved');
    expect(screen.getByText(/Looks like this may contain: JWT · API key/)).toBeDefined();
    expect(screen.getByText(/only a suggestion/)).toBeDefined();
  });

  it('chooses collection and expansion as trace options and saves them', async () => {
    renderAt('/editor/s-1');
    await flush();
    fireEvent.click(screen.getByRole('radio', { name: 'Infra' }));
    fireEvent.click(screen.getByRole('radio', { name: 'At once' }));
    fireEvent.click(screen.getByLabelText('Pinned'));
    await settleAutosave();
    expect(updateSnippet.mock.calls.at(-1)?.[0]).toMatchObject({
      folderId: 'f-1',
      triggerMode: 'immediate',
      isPinned: true,
    });
  });

  it('keeps the no-clash note steady while other settings autosave', async () => {
    renderAt('/editor/s-1');
    await flush();
    const note = 'Type these 3 characters and it expands. No other snippet uses them.';
    expect(screen.getByText(note)).toBeDefined();

    fireEvent.click(screen.getByRole('radio', { name: 'At once' }));
    expect(screen.getByText(note)).toBeDefined();
    await settleAutosave();
    expect(screen.getByText(note)).toBeDefined();

    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: ';hey' } });
    expect(screen.getByText('Type these 4 characters and it expands.')).toBeDefined();
    await settleAutosave();
    expect(
      screen.getByText('Type these 4 characters and it expands. No other snippet uses them.'),
    ).toBeDefined();
  });

  it('moves a saved snippet into the vault once the vault is unlocked', async () => {
    vaultStatusMock.mockResolvedValue({
      initialized: true,
      unlocked: true,
      unlockedAt: 1,
      lastActivityAt: 1,
      idleTimeoutMs: 300_000,
    });
    convertToSensitive.mockResolvedValue({
      ...EXISTING,
      snippetType: 'sensitive',
      securityLevel: 'sensitive',
      body: null,
    });
    renderAt('/editor/s-1');
    await flush();
    await flush();

    fireEvent.click(screen.getByText('Move to the vault'));
    // Confirmation is inline — no modal.
    fireEvent.click(screen.getByText('Move to vault'));
    await flush();

    expect(convertToSensitive).toHaveBeenCalledWith('s-1');
    expect(screen.getByText('Vault landing')).toBeDefined();
  });

  it('will not offer the vault move while the vault is locked', async () => {
    renderAt('/editor/s-1');
    await flush();
    await flush();
    expect(screen.queryByText('Move to the vault')).toBeNull();
    expect(screen.getByText(/Unlock the vault to move this in/)).toBeDefined();
  });

  it('surfaces a trigger conflict under the trigger without losing the draft', async () => {
    updateSnippet.mockRejectedValue(new IpcError('conflict', 'trigger already in use'));
    renderAt('/editor/s-1');
    await flush();
    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: ';taken' } });
    await settleAutosave();

    expect(screen.getByRole('status').textContent).toBe('Not saved');
    expect(screen.getByRole('alert').textContent).toBe('trigger already in use');
    expect(screen.getByLabelText<HTMLInputElement>('Trigger').value).toBe(';taken');
  });

  it('locks sensitive snippets out of the editor and points to the vault', async () => {
    renderAt('/editor/s-sec');
    await flush();
    expect(
      screen.getByText(
        'Still safely encrypted — secret snippets are viewed and edited in the Vault.',
      ),
    ).toBeDefined();
    expect(screen.queryByLabelText('Snippet body')).toBeNull();
    expect(screen.queryByLabelText('Edit snippet body')).toBeNull();

    fireEvent.click(screen.getByText('Open Vault'));
    await flush();
    expect(screen.getByText('Vault landing')).toBeDefined();
  });
});
