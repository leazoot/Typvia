// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { Snippet, VaultStatus } from '@typvia/shared';
import type * as SharedModule from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VaultProvider } from '../../vault/vault-context';
import { UndoProvider } from '../../workspace/undo';
import { VaultPage } from './vault-page';

const mocks = vi.hoisted(() => ({
  vaultStatus: vi.fn(),
  vaultList: vi.fn(),
  vaultReveal: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockBiometric: vi.fn(),
  vaultLock: vi.fn(),
  vaultInitialize: vi.fn(),
  vaultCreateSecret: vi.fn(),
  vaultUpdateSecret: vi.fn(),
  vaultEnableBiometric: vi.fn(),
  vaultResetPreview: vi.fn(),
  vaultReset: vi.fn(),
  panelCopySecret: vi.fn(),
  trashSnippet: vi.fn(),
  restoreSnippet: vi.fn(),
  masterPasswordMinLength: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...mocks };
});

function status(overrides: Partial<VaultStatus>): VaultStatus {
  return {
    initialized: true,
    unlocked: false,
    unlockedAt: null,
    lastActivityAt: null,
    idleTimeoutMs: 600_000,
    ...overrides,
  };
}

const unlocked = () =>
  status({ unlocked: true, unlockedAt: Date.now(), lastActivityAt: Date.now() });

function secret(id: string, title: string): Snippet {
  return {
    id,
    title,
    body: null,
    snippetType: 'sensitive',
    securityLevel: 'sensitive',
    description: null,
    folderId: null,
    trigger: `/${id}`,
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
    deletedAt: null,
  };
}

function renderVault(locale: 'en' | 'zh' = 'en') {
  mocks.masterPasswordMinLength.mockResolvedValue(8);
  return render(
    <I18nProvider locale={locale}>
      <MemoryRouter initialEntries={['/vault']}>
        <VaultProvider>
          <UndoProvider>
            <VaultPage />
          </UndoProvider>
        </VaultProvider>
      </MemoryRouter>
    </I18nProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('VaultPage — locked and not set up', () => {
  it('shows the set-up lines when no vault exists', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: false }));
    renderVault();
    expect(await screen.findByRole('heading', { name: 'Set up the vault' })).toBeTruthy();
    expect(screen.getByLabelText('Confirm master password')).toBeTruthy();
  });

  it('locks in place with the count, the caret in the password line, and no titles', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod DB'), secret('b', 'Stripe')]);
    renderVault();
    expect(
      await screen.findByRole('heading', { level: 1, name: '2 snippets are locked in here.' }),
    ).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByLabelText('Master password'));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.queryByText('Prod DB')).toBeNull();
    expect(screen.getByText(/locks itself again after 10 minutes idle/)).toBeTruthy();
  });

  it('resets the vault from the lost-password flow with the real count', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod DB')]);
    mocks.vaultResetPreview.mockResolvedValue(3);
    mocks.vaultReset.mockResolvedValue(status({ initialized: false, unlocked: false }));
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'Forgot the master password?' }));
    // Two presses: the first names the act, the second names the real count.
    fireEvent.click(await screen.findByRole('button', { name: 'Reset the vault' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Destroy 3 secret snippets forever' }),
    );

    await waitFor(() => expect(mocks.vaultReset).toHaveBeenCalled());
    expect(await screen.findByRole('heading', { name: 'Set up the vault' })).toBeTruthy();
  });

  it('unlocks with the master password and then lists secrets', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod read replica')]);
    mocks.vaultUnlockPassword.mockResolvedValue(unlocked());
    renderVault();

    const field = await screen.findByLabelText('Master password');
    fireEvent.change(field, { target: { value: 'correct horse battery staple' } });
    const form = field.closest('form');
    if (form === null) throw new Error('the password line is not in a form');
    fireEvent.submit(form);

    await waitFor(() =>
      expect(mocks.vaultUnlockPassword).toHaveBeenCalledWith('correct horse battery staple'),
    );
    expect(await screen.findByText('Prod read replica')).toBeTruthy();
  });

  it('renders a single language (zh)', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: false }));
    renderVault('zh');
    expect(await screen.findByText('设置保险库')).toBeTruthy();
    expect(screen.queryByText('Set up the vault')).toBeNull();
  });
});

describe('VaultPage — unlocked', () => {
  it('peeks at a secret only on request and covers it again', async () => {
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.vaultReveal.mockResolvedValue('sk_test_51NfEXAMPLEONLY');
    renderVault();

    await screen.findByText('Stripe test key');
    expect(screen.queryByText('sk_test_51NfEXAMPLEONLY')).toBeNull();
    expect(screen.getByText(/^Unlocked \d+:\d\d$/)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Peek' }));
    expect(await screen.findByText('sk_test_51NfEXAMPLEONLY')).toBeTruthy();
    expect(screen.getByText('covers again in 10s')).toBeTruthy();
    expect(mocks.vaultReveal).toHaveBeenCalledWith('a');

    fireEvent.click(screen.getByRole('button', { name: 'Cover' }));
    await waitFor(() => expect(screen.queryByText('sk_test_51NfEXAMPLEONLY')).toBeNull());
  });

  it('edits an existing secret by decrypting it into the form', async () => {
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.vaultReveal.mockResolvedValue('sk_test_old');
    mocks.vaultUpdateSecret.mockResolvedValue(secret('a', 'Stripe test key'));
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    const bodyField = await screen.findByLabelText<HTMLTextAreaElement>('Secret value');
    await waitFor(() => expect(bodyField.value).toBe('sk_test_old'));

    fireEvent.change(bodyField, { target: { value: 'sk_test_new' } });
    fireEvent.click(screen.getByText('Save'));

    await waitFor(() =>
      expect(mocks.vaultUpdateSecret).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'a', body: 'sk_test_new', snippetType: 'sensitive' }),
      ),
    );
  });

  it('copies a secret with the clipboard-clear promise, and never shows it', async () => {
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.panelCopySecret.mockResolvedValue(30_000);
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(mocks.panelCopySecret).toHaveBeenCalledWith('a'));
    expect(await screen.findByText('Clipboard clears in 30s')).toBeTruthy();
    expect(mocks.vaultReveal).not.toHaveBeenCalled();
  });

  it('deletes from the right-click menu and offers it back on a note', async () => {
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.trashSnippet.mockResolvedValue(undefined);
    mocks.restoreSnippet.mockResolvedValue(undefined);
    renderVault();

    fireEvent.contextMenu(await screen.findByRole('listitem', { name: 'Stripe test key' }), {
      clientX: 10,
      clientY: 10,
    });
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete' }));
    await waitFor(() => expect(mocks.trashSnippet).toHaveBeenCalledWith('a'));

    const note = await screen.findByRole('status', { name: 'Deleted “Stripe test key”.' });
    fireEvent.click(within(note).getByRole('button', { name: 'Undo ⌘Z' }));
    await waitFor(() => expect(mocks.restoreSnippet).toHaveBeenCalledWith('a'));
  });

  it('offers Touch ID once and then stops asking', async () => {
    localStorage.removeItem('tv.ui.vaultTouchIdAsked');
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([]);
    mocks.vaultEnableBiometric.mockResolvedValue(undefined);
    renderVault();

    expect(await screen.findByText('Use Touch ID next time?')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
    await waitFor(() => expect(mocks.vaultEnableBiometric).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText('Use Touch ID next time?')).toBeNull());
    expect(localStorage.getItem('tv.ui.vaultTouchIdAsked')).toBe('true');
    expect(screen.getByText('Touch ID can unlock this vault.')).toBeTruthy();
  });

  it('creates a secret through the New secret form', async () => {
    mocks.vaultStatus.mockResolvedValue(unlocked());
    mocks.vaultList.mockResolvedValue([]);
    mocks.vaultCreateSecret.mockResolvedValue(secret('new', 'API key'));
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'New secret' }));
    fireEvent.change(screen.getByLabelText('Secret title'), { target: { value: 'API key' } });
    fireEvent.change(screen.getByLabelText('Secret value'), { target: { value: 'FAKE_s3cr3t' } });
    fireEvent.click(screen.getByText('Save secret'));

    await waitFor(() =>
      expect(mocks.vaultCreateSecret).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'API key',
          body: 'FAKE_s3cr3t',
          snippetType: 'sensitive',
        }),
      ),
    );
  });
});
