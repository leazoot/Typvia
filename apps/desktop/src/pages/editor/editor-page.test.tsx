// @vitest-environment jsdom
import { IpcError } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
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

const createSnippet = vi.fn();
const updateSnippet = vi.fn();
const detectSensitive = vi.fn();
const convertToSensitive = vi.fn();
const vaultStatusMock = vi.fn();

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
    listFolderChildren: (parentId: string | null) =>
      Promise.resolve(
        parentId === null
          ? [{ id: 'f-1', parentId: null, name: 'Infra', sortOrder: 0, createdAt: 1, updatedAt: 1 }]
          : [],
      ),
  };
});

function renderNew() {
  return render(
    <MemoryRouter initialEntries={['/editor']}>
      <Routes>
        <Route path="/editor" element={<EditorPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

function renderExisting() {
  return render(
    <MemoryRouter initialEntries={['/editor/s-1']}>
      <Routes>
        <Route path="/editor/:id" element={<EditorPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

function renderExistingWithVault() {
  return render(
    <MemoryRouter initialEntries={['/editor/s-1']}>
      <VaultProvider>
        <Routes>
          <Route path="/editor/:id" element={<EditorPage />} />
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

describe('EditorPage', () => {
  it('auto-saves a new draft once a title exists, then shows Saved', async () => {
    renderNew();
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
    // The auto-save fires mid-typing, so
    // it must never remount the editor — a remount steals focus and forces
    // a click back into the field after every save.
    renderNew();
    fireEvent.change(screen.getByLabelText('Snippet title'), {
      target: { value: 'Deploy checklist' },
    });
    const body = screen.getByLabelText('Snippet body');
    body.focus();
    fireEvent.change(body, { target: { value: 'kubectl get pods' } });
    await settleAutosave();

    expect(createSnippet).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(screen.getByLabelText('Snippet body'));
    // A second save cycle (the update path) must hold focus too.
    fireEvent.change(screen.getByLabelText('Snippet body'), {
      target: { value: 'kubectl get pods -A' },
    });
    await settleAutosave();
    expect(document.activeElement).toBe(screen.getByLabelText('Snippet body'));
  });

  it('does not try to save while the title is blank', async () => {
    renderNew();
    fireEvent.change(screen.getByLabelText('Snippet body'), { target: { value: 'body only' } });
    await settleAutosave();
    expect(createSnippet).not.toHaveBeenCalled();
    expect(screen.getByRole('status').textContent).toBe('Draft');
  });

  it('auto-saves edits to an existing snippet and reflects the bumped version', async () => {
    renderExisting();
    await flush();
    const title = screen.getByLabelText('Snippet title');
    expect((title as HTMLInputElement).value).toBe('Existing snippet');

    fireEvent.change(title, { target: { value: 'Existing snippet, renamed' } });
    await settleAutosave();

    expect(updateSnippet).toHaveBeenCalledTimes(1);
    expect(updateSnippet.mock.calls[0]?.[0]).toMatchObject({
      id: 's-1',
      title: 'Existing snippet, renamed',
    });
    expect(screen.getByRole('status').textContent).toContain('Saved · v4');
  });

  it('renders {{variables}} as inline tokens and lists them in the Fields tab', async () => {
    const { container } = renderExisting();
    await flush();
    const tokens = container.querySelectorAll('.tv-ed-token');
    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens[0]?.textContent).toBe('{{name}}');
  });

  it('shows the sensitive hint as advice while the save still succeeds', async () => {
    detectSensitive.mockResolvedValue(['jwt', 'api_key']);
    renderExisting();
    await flush();
    const title = screen.getByLabelText('Snippet title');
    fireEvent.change(title, { target: { value: 'Now with a token' } });
    await settleAutosave();

    expect(screen.getByRole('status').textContent).toContain('Saved');
    fireEvent.click(screen.getByRole('tab', { name: 'Security' }));
    expect(screen.getByText(/Looks like this may contain: JWT · API key/)).toBeDefined();
    expect(screen.getByText(/only a suggestion/)).toBeDefined();
  });

  it('renders only the open property tab', async () => {
    renderExisting();
    await flush();
    // Fields is open by default; the usage tab's fields are not in the DOM.
    expect(screen.queryByLabelText('Trigger')).toBeNull();
    fireEvent.click(screen.getByRole('tab', { name: 'Where it can be used' }));
    expect(screen.getByLabelText('Trigger')).toBeDefined();
    expect(screen.queryByText(/Field settings/)).toBeNull();
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
    renderExistingWithVault();
    await flush();
    await flush();

    fireEvent.click(screen.getByRole('tab', { name: 'Security' }));
    fireEvent.click(screen.getByText('Move to the vault'));
    // Confirmation is inline — no modal.
    fireEvent.click(screen.getByText('Move to vault'));
    await flush();

    expect(convertToSensitive).toHaveBeenCalledWith('s-1');
    // Landed on the vault after the move.
    expect(screen.getByText('Vault landing')).toBeDefined();
  });

  it('will not offer the vault move while the vault is locked', async () => {
    renderExistingWithVault();
    await flush();
    await flush();

    fireEvent.click(screen.getByRole('tab', { name: 'Security' }));
    expect(screen.queryByText('Move to the vault')).toBeNull();
    expect(screen.getByText(/Unlock the vault to move this in/)).toBeDefined();
  });

  it('surfaces a trigger conflict as a business error without losing the draft', async () => {
    updateSnippet.mockRejectedValue(new IpcError('conflict', 'trigger already in use'));
    renderExisting();
    await flush();
    fireEvent.click(screen.getByRole('tab', { name: 'Where it can be used' }));
    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: ';taken' } });
    await settleAutosave();

    expect(screen.getByRole('status').textContent).toBe('Not saved');
    expect(screen.getByText('trigger already in use')).toBeDefined();
    expect((screen.getByLabelText('Trigger') as HTMLInputElement).value).toBe(';taken');
  });

  it('locks sensitive snippets out of the editor and points to the vault', async () => {
    render(
      <MemoryRouter initialEntries={['/editor/s-sec']}>
        <Routes>
          <Route path="/editor/:id" element={<EditorPage />} />
          <Route path="/vault" element={<div>Vault landing</div>} />
        </Routes>
      </MemoryRouter>,
    );
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
