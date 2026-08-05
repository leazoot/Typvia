// @vitest-environment jsdom
import { IpcError } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
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

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    getSnippet: (id: string) =>
      id === 's-1'
        ? Promise.resolve(EXISTING)
        : Promise.reject(new actual.IpcError('not_found', 'not found')),
    createSnippet: (...args: unknown[]) => createSnippet(...args) as Promise<Snippet>,
    updateSnippet: (...args: unknown[]) => updateSnippet(...args) as Promise<Snippet>,
    detectSensitive: (...args: unknown[]) => detectSensitive(...args) as Promise<string[]>,
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
    expect(screen.queryByText(/字段配置/)).toBeNull();
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
});
