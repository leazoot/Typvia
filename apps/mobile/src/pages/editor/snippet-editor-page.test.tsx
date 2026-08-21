// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet } from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { installAndroidBackBridge } from '../../platform';
import { SnippetEditorPage } from './snippet-editor-page';

const ipc = vi.hoisted(() => ({
  createSnippet: vi.fn(),
  updateSnippet: vi.fn(),
  vaultCreateSecret: vi.fn(),
  historyList: vi.fn(),
  listFolderChildren: vi.fn(),
  aiProviderList: vi.fn(),
  aiActionList: vi.fn(),
  aiActionRun: vi.fn(),
  aiOrganize: vi.fn(),
  batchTagSnippets: vi.fn(),
}));

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  ...ipc,
}));

function snippet(overrides: Partial<Snippet> = {}): Snippet {
  return {
    id: 's-1',
    title: 'Tail prod API logs',
    body: 'kubectl logs -f deploy/api -n prod --tail=100',
    snippetType: 'command',
    securityLevel: 'normal',
    description: null,
    folderId: null,
    trigger: ';klogs',
    triggerMode: 'delimiter',
    language: null,
    isFavorite: true,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 1,
    lastUsedAt: null,
    usageCount: 4,
    version: 3,
    deletedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  ipc.listFolderChildren.mockResolvedValue([]);
  ipc.historyList.mockResolvedValue({ current: 1, entries: [] });
  ipc.createSnippet.mockResolvedValue(snippet());
  ipc.updateSnippet.mockResolvedValue(snippet());
  // AI off by default: the strip stays hidden in the existing scenarios.
  ipc.aiProviderList.mockResolvedValue([]);
  ipc.aiActionList.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  delete window.__TYPVIA_ANDROID_BACK__;
});

describe('SnippetEditorPage — create', () => {
  it('opens as the quick sheet: Save disabled, fields hidden until content', () => {
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    expect(screen.getByRole('dialog', { name: 'New snippet' })).toBeDefined();
    expect(screen.getByText('Save something.')).toBeDefined();
    const save = screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    // Quick state: content first, no fields yet.
    expect(screen.queryByLabelText('Title')).toBeNull();
  });

  it('expands into the field set once content exists and enables Save', () => {
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'ok' } });
    expect(screen.getByLabelText('Title')).toBeDefined();
    expect(screen.getByLabelText('Trigger')).toBeDefined();
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('derives the title from the first content line when the title is left empty', async () => {
    const longFirstLine = `docker run --rm -it -v $(pwd):/work -w /work node:22 bash -c "pnpm i"`;
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: `${longFirstLine}\nsecond line` },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(ipc.createSnippet).toHaveBeenCalledTimes(1);
    });
    const input = ipc.createSnippet.mock.calls[0]?.[0] as { title: string; body: string };
    expect(input.title).toBe(`${longFirstLine.slice(0, 59)}…`);
    expect(input.title.length).toBe(60);
    expect(input.body).toContain('second line');
  });

  it('sends the chosen type from the underline tabs', async () => {
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'kubectl' } });
    expect(screen.getByRole('button', { name: 'Text' }).getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'Command' }));
    expect(screen.getByRole('button', { name: 'Command' }).getAttribute('aria-pressed')).toBe(
      'true',
    );

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(ipc.createSnippet).toHaveBeenCalledWith(
        expect.objectContaining({ snippetType: 'command' }),
      );
    });
  });

  it('saves a Secret through the vault, never the plain create path', async () => {
    ipc.vaultCreateSecret.mockResolvedValue(
      snippet({ id: 's-secret', snippetType: 'sensitive', securityLevel: 'sensitive' }),
    );
    const onSaved = vi.fn();
    render(<SnippetEditorPage onCancel={() => {}} onSaved={onSaved} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'hunter2' } });
    fireEvent.click(screen.getByRole('button', { name: 'Secret' }));
    // No trigger field for a secret: triggers cannot expand vault content.
    expect(screen.queryByLabelText('Trigger')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(onSaved).toHaveBeenCalledTimes(1);
    });
    expect(ipc.vaultCreateSecret).toHaveBeenCalledWith(
      expect.objectContaining({ snippetType: 'sensitive', body: 'hunter2' }),
    );
    expect(ipc.createSnippet).not.toHaveBeenCalled();
  });

  it('offers the folder roots and sends the chosen folder id', async () => {
    ipc.listFolderChildren.mockResolvedValue([
      { id: 'f-1', parentId: null, name: 'Infra', sortOrder: 0, createdAt: 1, updatedAt: 1 },
    ]);
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'body' } });
    const select = await screen.findByLabelText('Folder');
    fireEvent.change(select, { target: { value: 'f-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(ipc.createSnippet).toHaveBeenCalledWith(expect.objectContaining({ folderId: 'f-1' }));
    });
  });

  it('shows the folder row as unavailable when no folders exist', async () => {
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'body' } });
    expect(await screen.findByText('No folders yet')).toBeDefined();
    expect(screen.queryByLabelText('Folder')).toBeNull();
  });

  it('submits the trimmed trigger and renders a core rejection verbatim, draft intact', async () => {
    ipc.createSnippet.mockRejectedValue(new Error('trigger ";klogs" is already in use'));
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'kubectl' } });
    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: ' ;klogs ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const alert = await screen.findByRole('alert');
    // Failure copy names what is still fine first.
    expect(alert.textContent).toContain('Nothing was saved — your draft is intact.');
    expect(alert.textContent).toContain('trigger ";klogs" is already in use');
    expect(ipc.createSnippet).toHaveBeenCalledWith(expect.objectContaining({ trigger: ';klogs' }));
    // The draft really is intact and the page is still editable.
    expect((screen.getByLabelText('Snippet content') as HTMLTextAreaElement).value).toBe('kubectl');
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it('routes the Android system back gesture to the cancel path while open', () => {
    installAndroidBackBridge();
    const onCancel = vi.fn();
    const { unmount } = render(<SnippetEditorPage onCancel={onCancel} onSaved={() => {}} />);

    expect(window.__TYPVIA_ANDROID_BACK__?.()).toBe(true);
    expect(onCancel).toHaveBeenCalledTimes(1);

    // A closed editor releases the gesture back to the host default.
    unmount();
    expect(window.__TYPVIA_ANDROID_BACK__?.()).toBe(false);
  });

  it('hands the saved snippet to the host on success', async () => {
    const saved = snippet({ id: 's-9' });
    ipc.createSnippet.mockResolvedValue(saved);
    const onSaved = vi.fn();
    render(<SnippetEditorPage onCancel={() => {}} onSaved={onSaved} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'kubectl' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(onSaved).toHaveBeenCalledWith(saved);
    });
    // The empty trigger travels as null, never as an empty string.
    expect(ipc.createSnippet).toHaveBeenCalledWith(expect.objectContaining({ trigger: null }));
  });

  // Regression (found on-device): core rejects a trigger without a mode —
  // "trigger and trigger_mode must be set together".
  it('pairs a trigger with the immediate mode and clears the mode with the trigger', async () => {
    ipc.createSnippet.mockResolvedValue(snippet({ id: 's-10' }));
    const { unmount } = render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'kubectl' } });
    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: ';klogs' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(ipc.createSnippet).toHaveBeenCalledWith(
        expect.objectContaining({ trigger: ';klogs', triggerMode: 'immediate' }),
      );
    });
    unmount();

    // Clearing the trigger on an edit clears the stored mode with it.
    const existing = snippet({ id: 's-11', trigger: ';old', triggerMode: 'immediate' });
    ipc.updateSnippet.mockResolvedValue(existing);
    render(<SnippetEditorPage snippet={existing} onCancel={() => {}} onSaved={() => {}} />);
    fireEvent.change(screen.getByLabelText('Trigger'), { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(ipc.updateSnippet).toHaveBeenCalledWith(
        expect.objectContaining({ trigger: null, triggerMode: null }),
      );
    });
  });

  it('saves on Cmd/Ctrl+Enter and cancels on Escape', async () => {
    const onCancel = vi.fn();
    render(<SnippetEditorPage onCancel={onCancel} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'kubectl' } });
    fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
    await waitFor(() => {
      expect(ipc.createSnippet).toHaveBeenCalledTimes(1);
    });

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('cancels from the header without saving anything', () => {
    const onCancel = vi.fn();
    render(<SnippetEditorPage onCancel={onCancel} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), { target: { value: 'draft' } });
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(ipc.createSnippet).not.toHaveBeenCalled();
  });
});

describe('SnippetEditorPage — edit', () => {
  it('pre-fills every field from the snippet DTO', () => {
    render(<SnippetEditorPage snippet={snippet()} onCancel={() => {}} onSaved={() => {}} />);

    expect(screen.getByText('Edit snippet.')).toBeDefined();
    expect((screen.getByLabelText('Snippet content') as HTMLTextAreaElement).value).toBe(
      'kubectl logs -f deploy/api -n prod --tail=100',
    );
    expect((screen.getByLabelText('Title') as HTMLInputElement).value).toBe('Tail prod API logs');
    expect((screen.getByLabelText('Trigger') as HTMLInputElement).value).toBe(';klogs');
    expect(screen.getByRole('button', { name: 'Command' }).getAttribute('aria-pressed')).toBe(
      'true',
    );
  });

  it('updates with the snippet id and the full field set', async () => {
    render(<SnippetEditorPage snippet={snippet()} onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: 'kubectl logs -f deploy/api' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(ipc.updateSnippet).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 's-1',
          body: 'kubectl logs -f deploy/api',
          title: 'Tail prod API logs',
          snippetType: 'command',
          trigger: ';klogs',
          // Flags the page does not surface travel through unchanged.
          isFavorite: true,
          isPinned: false,
          isEnabled: true,
        }),
      );
    });
  });

  it('renders the version list rows from historyList — no diff on mobile', async () => {
    ipc.historyList.mockResolvedValue({
      current: 3,
      entries: [
        { version: 3, title: 'Tail prod API logs', createdAt: Date.UTC(2026, 7, 4, 12) },
        { version: 2, title: 'Tail prod API logs', createdAt: Date.UTC(2026, 6, 30, 12) },
      ],
    });
    render(<SnippetEditorPage snippet={snippet()} onCancel={() => {}} onSaved={() => {}} />);

    // The versions sit behind the More options fold.
    fireEvent.click(screen.getByRole('button', { name: /More options/ }));
    expect(await screen.findByText('v3')).toBeDefined();
    expect(screen.getByText('v2')).toBeDefined();
    expect(screen.getByText(/current/)).toBeDefined();
    expect(ipc.historyList).toHaveBeenCalledWith('s-1', 20, 0);
  });

  it('says the snippet is unaffected when the history cannot load', async () => {
    ipc.historyList.mockRejectedValue(new Error('io'));
    render(<SnippetEditorPage snippet={snippet()} onCancel={() => {}} onSaved={() => {}} />);

    fireEvent.click(screen.getByRole('button', { name: /More options/ }));
    expect(
      await screen.findByText(
        'The versions could not be loaded — the snippet itself is unaffected.',
      ),
    ).toBeDefined();
  });

  it('never asks for history in create mode', () => {
    render(<SnippetEditorPage onCancel={() => {}} onSaved={() => {}} />);

    expect(ipc.historyList).not.toHaveBeenCalled();
    expect(screen.queryByText('History')).toBeNull();
  });
});
