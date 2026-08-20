// @vitest-environment jsdom
import type { Snippet, VaultStatus } from '@typvia/shared';
import type * as SharedModule from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VaultProvider } from '../../vault/vault-context';
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
    idleTimeoutMs: 300_000,
    ...overrides,
  };
}

function secret(id: string, title: string): Snippet {
  return {
    id,
    title,
    body: null,
    snippetType: 'sensitive',
    securityLevel: 'sensitive',
    description: null,
    folderId: null,
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
    deletedAt: null,
  };
}

function renderVault() {
  return render(
    <MemoryRouter initialEntries={['/vault']}>
      <VaultProvider>
        <VaultPage />
      </VaultProvider>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('VaultPage', () => {
  it('shows the set-up form when no vault exists', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: false }));
    renderVault();
    expect(await screen.findByText('Set up the vault')).toBeTruthy();
  });

  it('locks in place: the page keeps its shape and lists nothing', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: true, unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod DB'), secret('b', 'Stripe')]);
    renderVault();
    expect(await screen.findByText('2 secrets are kept safe on this Mac.')).toBeTruthy();
    // The vault heading stays; the lock is a state of the page, not a modal.
    expect(screen.getByRole('heading', { level: 1, name: 'Vault' })).toBeTruthy();
    expect(screen.queryByRole('dialog')).toBeNull();
    // Titles are not listed while locked, and no secret value appears.
    expect(screen.queryByText('Prod DB')).toBeNull();
  });

  it('resets the vault from the lost-password flow with the real count', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: true, unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod DB')]);
    mocks.vaultResetPreview.mockResolvedValue(3);
    mocks.vaultReset.mockResolvedValue(status({ initialized: false, unlocked: false }));
    renderVault();

    fireEvent.click(await screen.findByText('Forgot the master password?'));
    // Two steps: the first names the action, the confirm names the real count.
    fireEvent.click(await screen.findByRole('button', { name: 'Reset the vault' }));
    fireEvent.click(await screen.findByText('Destroy 3 secret snippets forever'));

    await waitFor(() => expect(mocks.vaultReset).toHaveBeenCalled());
    // The fresh uninitialized status drops straight into first-run setup.
    expect(await screen.findByText('Set up the vault')).toBeTruthy();
  });

  it('unlocks with the master password and then lists secrets', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ unlocked: false }));
    mocks.vaultList.mockResolvedValue([secret('a', 'Prod read replica')]);
    mocks.vaultUnlockPassword.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    renderVault();

    fireEvent.click(await screen.findByText('Master password'));
    fireEvent.change(screen.getByLabelText('Master password'), {
      target: { value: 'correct horse battery staple' },
    });
    fireEvent.click(screen.getByText('Unlock'));

    await waitFor(() =>
      expect(mocks.vaultUnlockPassword).toHaveBeenCalledWith('correct horse battery staple'),
    );
    expect(await screen.findByText('Prod read replica')).toBeTruthy();
  });

  it('reveals a secret only on request and hides it otherwise', async () => {
    mocks.vaultStatus.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.vaultReveal.mockResolvedValue('sk_test_51NfEXAMPLEONLY');
    renderVault();

    // Before reveal, the plaintext is absent from the DOM.
    await screen.findByText('Stripe test key');
    expect(screen.queryByText('sk_test_51NfEXAMPLEONLY')).toBeNull();

    fireEvent.click(screen.getByText('Reveal'));
    expect(await screen.findByText('sk_test_51NfEXAMPLEONLY')).toBeTruthy();
    expect(mocks.vaultReveal).toHaveBeenCalledWith('a');

    // Hiding removes it again.
    fireEvent.click(screen.getByText('Hide'));
    await waitFor(() => expect(screen.queryByText('sk_test_51NfEXAMPLEONLY')).toBeNull());
  });

  it('edits an existing secret by decrypting it into the form', async () => {
    mocks.vaultStatus.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.vaultReveal.mockResolvedValue('sk_test_old');
    mocks.vaultUpdateSecret.mockResolvedValue(secret('a', 'Stripe test key'));
    renderVault();

    // Editing lives behind ··· , never as a permanent button on the row.
    fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
    // The edit session decrypts the current value into the field.
    const bodyField = (await screen.findByLabelText('Secret value')) as HTMLTextAreaElement;
    await waitFor(() => expect(bodyField.value).toBe('sk_test_old'));

    fireEvent.change(bodyField, { target: { value: 'sk_test_new' } });
    fireEvent.click(screen.getByText('Save'));

    await waitFor(() =>
      expect(mocks.vaultUpdateSecret).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'a', body: 'sk_test_new', snippetType: 'sensitive' }),
      ),
    );
  });

  it('offers Touch ID once and then stops asking', async () => {
    localStorage.removeItem('tv.ui.vaultTouchIdAsked');
    mocks.vaultStatus.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    mocks.vaultList.mockResolvedValue([]);
    mocks.vaultEnableBiometric.mockResolvedValue(undefined);
    renderVault();

    fireEvent.click(await screen.findByText('Use Touch ID next time?'));
    fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
    await waitFor(() => expect(mocks.vaultEnableBiometric).toHaveBeenCalled());
    // The offer is gone for good; the setting still lives in Security.
    await waitFor(() => expect(screen.queryByText('Use Touch ID next time?')).toBeNull());
    expect(localStorage.getItem('tv.ui.vaultTouchIdAsked')).toBe('true');

    fireEvent.click(screen.getByRole('button', { name: /Security/ }));
    expect(screen.getByText('Touch ID can unlock this vault.')).toBeTruthy();
  });

  it('copies a secret with the clipboard-clear promise, and never shows it', async () => {
    mocks.vaultStatus.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    mocks.vaultList.mockResolvedValue([secret('a', 'Stripe test key')]);
    mocks.panelCopySecret.mockResolvedValue(30_000);
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'Copy' }));
    await waitFor(() => expect(mocks.panelCopySecret).toHaveBeenCalledWith('a'));
    expect(await screen.findByText('Clipboard clears in 30s')).toBeTruthy();
    // Copying never decrypts into the page.
    expect(mocks.vaultReveal).not.toHaveBeenCalled();
  });

  it('renders a single language (zh) under I18nProvider', async () => {
    mocks.vaultStatus.mockResolvedValue(status({ initialized: false }));
    render(
      <MemoryRouter initialEntries={['/vault']}>
        <VaultProvider>
          <I18nProvider locale="zh">
            <VaultPage />
          </I18nProvider>
        </VaultProvider>
      </MemoryRouter>,
    );
    expect(await screen.findByText('设置保险库')).toBeTruthy();
    // The retired bilingual twin never renders alongside it.
    expect(screen.queryByText('Set up the vault')).toBeNull();
  });

  it('creates a secret through the New secret form', async () => {
    mocks.vaultStatus.mockResolvedValue(
      status({ unlocked: true, unlockedAt: 1000, lastActivityAt: 1000 }),
    );
    mocks.vaultList.mockResolvedValue([]);
    mocks.vaultCreateSecret.mockResolvedValue(secret('new', 'API key'));
    renderVault();

    fireEvent.click(await screen.findByRole('button', { name: 'New secret' }));
    fireEvent.change(screen.getByLabelText('Secret title'), { target: { value: 'API key' } });
    fireEvent.change(screen.getByLabelText('Secret value'), { target: { value: 's3cr3t' } });
    fireEvent.click(screen.getByText('Save secret'));

    await waitFor(() =>
      expect(mocks.vaultCreateSecret).toHaveBeenCalledWith(
        expect.objectContaining({ title: 'API key', body: 's3cr3t', snippetType: 'sensitive' }),
      ),
    );
  });
});
