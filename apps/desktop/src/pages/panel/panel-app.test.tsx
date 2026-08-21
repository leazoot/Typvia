// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import { IpcError } from '@typvia/shared';
import type { Snippet, VaultStatus } from '@typvia/shared';
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
const panelResults = vi.fn(() =>
  Promise.resolve<SharedModule.PanelResults>({ rows: [], hiddenByRules: 0 }),
);
const templateFields = vi.fn((id: string) => {
  void id;
  return Promise.resolve<SharedModule.TemplateField[]>([]);
});
const panelInsertTemplate = vi.fn((id: string, values: Record<string, string>) => {
  void id;
  void values;
  return Promise.resolve();
});
const vaultStatusFn = vi.fn<() => Promise<VaultStatus>>();
const panelInsertSecret = vi.fn((id: string) => {
  void id;
  return Promise.resolve();
});
const panelCopySecret = vi.fn((id: string) => {
  void id;
  return Promise.resolve(30_000);
});
const vaultUnlockPassword = vi.fn<(pw: string) => Promise<VaultStatus>>();
const vaultUnlockBiometric = vi.fn<() => Promise<VaultStatus>>();

function status(unlocked: boolean): VaultStatus {
  return {
    initialized: true,
    unlocked,
    unlockedAt: unlocked ? 1 : null,
    lastActivityAt: unlocked ? 1 : null,
    idleTimeoutMs: 300_000,
  };
}

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    hidePanel: () => hidePanel(),
    panelReady: () => panelReady(),
    panelInsert: (id: string) => panelInsert(id),
    copySnippet: (id: string) => copySnippet(id),
    panelResults: (...args: unknown[]) => panelResults(...(args as [])),
    templateFields: (id: string) => templateFields(id),
    panelInsertTemplate: (id: string, values: Record<string, string>) =>
      panelInsertTemplate(id, values),
    vaultStatus: () => vaultStatusFn(),
    panelInsertSecret: (id: string) => panelInsertSecret(id),
    panelCopySecret: (id: string) => panelCopySecret(id),
    vaultUnlockPassword: (pw: string) => vaultUnlockPassword(pw),
    vaultUnlockBiometric: () => vaultUnlockBiometric(),
  };
});

function templateField(over: Partial<SharedModule.TemplateField> = {}): SharedModule.TemplateField {
  return {
    id: 'f1',
    name: 'module',
    label: 'Module',
    fieldType: 'single_line_text',
    defaultValue: null,
    options: [],
    validation: null,
    isRequired: true,
    sortOrder: 0,
    platformOverrides: null,
    ...over,
  };
}

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

function secret(over: Partial<Snippet> = {}): Snippet {
  return snip({
    id: 'sec-1',
    title: 'Prod DB',
    body: null,
    snippetType: 'sensitive',
    securityLevel: 'sensitive',
    trigger: null,
    ...over,
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  showHandler = null;
  panelResults.mockResolvedValue({ rows: [], hiddenByRules: 0 });
  templateFields.mockResolvedValue([]);
  vaultStatusFn.mockResolvedValue(status(false));
  panelInsertSecret.mockResolvedValue(undefined);
  panelCopySecret.mockResolvedValue(30_000);
  vaultUnlockPassword.mockResolvedValue(status(true));
  vaultUnlockBiometric.mockResolvedValue(status(true));
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

  it('dismisses when the scrim around the card is clicked', () => {
    render(<PanelApp />);
    fireEvent.mouseDown(screen.getByTestId('panel-scrim'));
    expect(hidePanel).toHaveBeenCalledTimes(1);
  });

  it('loads recent snippets and names the destination on summon', async () => {
    panelResults.mockResolvedValue({
      rows: [snip({ id: 's-1', title: 'Docker logs' })],
      hiddenByRules: 0,
    });
    render(<PanelApp />);
    await summon('VS Code');

    expect(panelResults).toHaveBeenCalledWith('', 20);
    expect(screen.getByText('→ VS Code')).toBeTruthy();
    expect(screen.getByText('Docker logs')).toBeTruthy();
  });

  it('replaces recent with ranked results as you type (no debounce)', async () => {
    render(<PanelApp />);
    await summon();

    panelResults.mockResolvedValue({
      rows: [snip({ id: 's-9', title: 'Search hit' })],
      hiddenByRules: 0,
    });
    await act(async () => {
      fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'sea' } });
    });

    expect(panelResults).toHaveBeenCalledWith('sea', 20);
    expect(screen.getByText('Search hit')).toBeTruthy();
  });

  it('states how many rows app rules hid for the destination', async () => {
    panelResults.mockResolvedValue({ rows: [snip({ id: 's-1' })], hiddenByRules: 2 });
    render(<PanelApp />);
    await summon('Chrome');

    expect(screen.getByText('2 hidden by app rules for Chrome')).toBeTruthy();
  });

  it('keeps the panel up with a notice when an app rule blocks the insert', async () => {
    panelResults.mockResolvedValue({ rows: [snip({ id: 's-1' })], hiddenByRules: 0 });
    panelInsert.mockRejectedValue(new IpcError('rule_blocked', 'an app rule hides this snippet'));
    render(<PanelApp />);
    await summon('Chrome');

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });

    expect(hidePanel).not.toHaveBeenCalled();
    expect(copySnippet).not.toHaveBeenCalled();
    expect(
      screen.getByText('Your snippet is safe — an app rule blocks inserting into Chrome.'),
    ).toBeTruthy();
  });

  it('inserts the selected snippet on Enter', async () => {
    panelResults.mockResolvedValue({
      rows: [snip({ id: 's-1' }), snip({ id: 's-2', title: 'Second' })],
      hiddenByRules: 0,
    });
    render(<PanelApp />);
    await summon();

    fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    expect(panelInsert).toHaveBeenCalledWith('s-1');
  });

  it('moves selection with ArrowDown before inserting', async () => {
    panelResults.mockResolvedValue({
      rows: [snip({ id: 's-1' }), snip({ id: 's-2', title: 'Second' })],
      hiddenByRules: 0,
    });
    render(<PanelApp />);
    await summon();

    const input = screen.getByLabelText('Search snippets');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(panelInsert).toHaveBeenCalledWith('s-2');
  });

  it('copies the selected snippet on Shift+Enter and hides', async () => {
    panelResults.mockResolvedValue({ rows: [snip({ id: 's-1' })], hiddenByRules: 0 });
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter', shiftKey: true });
    });
    expect(copySnippet).toHaveBeenCalledWith('s-1');
    expect(hidePanel).toHaveBeenCalled();
  });

  it('enters fill mode for a template and injects the filled values on Enter', async () => {
    panelResults.mockResolvedValue({
      rows: [snip({ id: 't-1', snippetType: 'template' })],
      hiddenByRules: 0,
    });
    templateFields.mockResolvedValue([templateField({ name: 'module', label: 'Module' })]);
    render(<PanelApp />);
    await summon();

    // ↵ on a template loads its fields and opens the fill form (not a direct insert).
    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });
    expect(panelInsert).not.toHaveBeenCalled();
    const field = await screen.findByPlaceholderText('');
    // Fill the required field, then ↵ renders + injects via the host.
    fireEvent.change(field, { target: { value: 'auth-service' } });
    await act(async () => {
      fireEvent.keyDown(field, { key: 'Enter' });
    });
    expect(panelInsertTemplate).toHaveBeenCalledWith('t-1', { module: 'auth-service' });
  });

  it('injects a variable-free template straight through without a form', async () => {
    panelResults.mockResolvedValue({
      rows: [snip({ id: 't-2', snippetType: 'template' })],
      hiddenByRules: 0,
    });
    templateFields.mockResolvedValue([]);
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });
    expect(panelInsertTemplate).toHaveBeenCalledWith('t-2', {});
  });

  it('verifies before injecting a sensitive snippet when the vault is locked', async () => {
    vaultStatusFn.mockResolvedValue(status(false));
    vaultUnlockPassword.mockResolvedValue(status(true));
    panelResults.mockResolvedValue({
      rows: [secret({ id: 'sec-1', title: 'Prod DB' })],
      hiddenByRules: 0,
    });
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });
    // The verify prompt replaces the list; nothing is delivered yet.
    expect(panelInsertSecret).not.toHaveBeenCalled();
    const field = await screen.findByLabelText('Master password');

    fireEvent.change(field, { target: { value: 'correct horse battery staple' } });
    await act(async () => {
      fireEvent.click(screen.getByText('Unlock'));
    });

    expect(vaultUnlockPassword).toHaveBeenCalledWith('correct horse battery staple');
    expect(panelInsertSecret).toHaveBeenCalledWith('sec-1');
  });

  it('injects a sensitive snippet directly when the vault is already unlocked', async () => {
    vaultStatusFn.mockResolvedValue(status(true));
    panelResults.mockResolvedValue({
      rows: [secret({ id: 'sec-2', title: 'API key' })],
      hiddenByRules: 0,
    });
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });

    expect(panelInsertSecret).toHaveBeenCalledWith('sec-2');
    // No unlock prompt when the session is already open.
    expect(screen.queryByLabelText('Master password')).toBeNull();
  });

  it('copies a sensitive snippet through the guarded clipboard path and hides', async () => {
    vaultStatusFn.mockResolvedValue(status(true));
    panelResults.mockResolvedValue({ rows: [secret({ id: 'sec-3' })], hiddenByRules: 0 });
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter', shiftKey: true });
    });

    expect(panelCopySecret).toHaveBeenCalledWith('sec-3');
    expect(hidePanel).toHaveBeenCalled();
  });

  it('renders a single language (zh) under I18nProvider', async () => {
    render(
      <I18nProvider locale="zh">
        <PanelApp />
      </I18nProvider>,
    );
    await summon(null);
    expect(screen.getByLabelText('搜索片段')).toBeTruthy();
    expect(screen.getByText('还没有片段')).toBeTruthy();
    // The English side never renders alongside the Chinese one.
    expect(screen.queryByText('No snippets yet')).toBeNull();
  });

  it('keeps the verify prompt with a generic message when an unlock fails', async () => {
    vaultStatusFn.mockResolvedValue(status(false));
    vaultUnlockPassword.mockRejectedValue(new IpcError('permission_denied', 'nope'));
    panelResults.mockResolvedValue({ rows: [secret({ id: 'sec-4' })], hiddenByRules: 0 });
    render(<PanelApp />);
    await summon();

    await act(async () => {
      fireEvent.keyDown(screen.getByLabelText('Search snippets'), { key: 'Enter' });
    });
    const field = await screen.findByLabelText('Master password');
    fireEvent.change(field, { target: { value: 'wrong' } });
    await act(async () => {
      fireEvent.click(screen.getByText('Unlock'));
    });

    expect(panelInsertSecret).not.toHaveBeenCalled();
    expect(screen.getByRole('alert').textContent).toContain('still safe');
    // The prompt stays put so the user can retry.
    expect(screen.getByLabelText('Master password')).toBeTruthy();
  });
});
