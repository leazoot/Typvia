// @vitest-environment jsdom
//
// Android composition of the shared vault page. The platform
// module is mocked wholesale so the same page renders its Android reading:
// the gate is named Fingerprint / 指纹 (never "Face ID"), the secure-field
// explainer states the Android cause (vault items never enter the keyboard),
// and the shared Copy 30s path renders its countdown from the wire value.
// The iOS composition keeps its own file (vault-page.test.tsx) untouched.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VaultPage } from './vault-page';

const ipc = vi.hoisted(() => ({
  vaultStatus: vi.fn(),
  vaultInitialize: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockBiometric: vi.fn(),
  vaultEnableBiometric: vi.fn(),
  vaultLock: vi.fn(),
  vaultList: vi.fn(),
  vaultReveal: vi.fn(),
  vaultCopySecret: vi.fn(),
  vaultCreateSecret: vi.fn(),
  vaultResetPreview: vi.fn(),
  vaultReset: vi.fn(),
  listFolderChildren: vi.fn(),
}));

vi.mock('@typvia/shared', () => ipc);

vi.mock('../../platform', () => ({
  isAndroid: true,
  biometricWord: 'Fingerprint',
  biometricWordZh: '指纹',
}));

const IDLE_TIMEOUT_MS = 5 * 60_000;

function status(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    initialized: true,
    unlocked: false,
    unlockedAt: null,
    lastActivityAt: null,
    idleTimeoutMs: IDLE_TIMEOUT_MS,
    ...overrides,
  };
}

function unlockedStatus() {
  const now = Date.now();
  return status({ unlocked: true, unlockedAt: now, lastActivityAt: now });
}

function secret(id: string, title: string) {
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

beforeEach(() => {
  // The unlocked view resolves folder names for its meta line; the rows in
  // these tests are folderless, so an empty listing is the honest default.
  ipc.listFolderChildren.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile vault page on Android', () => {
  it('offers the fingerprint unlock, never the Face ID wording', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    render(<VaultPage />);

    expect(await screen.findByText('Protected on this device')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Use Fingerprint' })).toBeDefined();
    expect(screen.queryByText(/Face ID/)).toBeNull();
    expect(screen.getByRole('button', { name: 'Enter passcode' })).toBeDefined();
  });

  it('unlocks through the BiometricPrompt-backed command', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    ipc.vaultList.mockResolvedValue([]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultUnlockBiometric.mockResolvedValue(unlockedStatus());
    render(<VaultPage />);
    await screen.findByText('Protected on this device');

    fireEvent.click(screen.getByRole('button', { name: 'Use Fingerprint' }));

    expect(await screen.findByText(/Nothing in the vault yet/)).toBeDefined();
    expect(ipc.vaultUnlockBiometric).toHaveBeenCalledTimes(1);
  });

  it('names the fingerprint gate in the enrollment strip in either language', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    ipc.vaultEnableBiometric.mockResolvedValue(undefined);
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    fireEvent.click(screen.getByRole('button', { name: 'Enable Fingerprint for faster unlock' }));
    expect(await screen.findByText(/Fingerprint is on for this vault/)).toBeDefined();

    // The Chinese reading uses the Android gate word too, with CJK spacing.
    cleanup();
    render(
      <I18nProvider locale="zh">
        <VaultPage />
      </I18nProvider>,
    );
    await screen.findByText(/保险库还是空的/);
    fireEvent.click(screen.getByRole('button', { name: '开启指纹以更快解锁' }));
    expect(await screen.findByText('已开启指纹解锁。')).toBeDefined();
  });

  it('keeps the password path when the fingerprint store is unavailable', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    ipc.vaultEnableBiometric.mockRejectedValue(new Error('unavailable'));
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    fireEvent.click(screen.getByRole('button', { name: 'Enable Fingerprint for faster unlock' }));

    expect(
      await screen.findByText(/Fingerprint isn’t available here — the master password still works/),
    ).toBeDefined();
  });

  it('explains the Android secure-field round trip with the Android cause', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    expect(screen.getByText('Secure field — the honest downgrade, in three steps')).toBeDefined();
    // The honest Android cause: secrets never ride the keyboard — never the
    // iOS sentence, never the iOS gate word.
    expect(screen.getByText(/Typvia’s keyboard never carries vault secrets/)).toBeDefined();
    expect(screen.queryByText(/iOS won’t let any third-party keyboard/)).toBeNull();
    expect(screen.queryByText(/Face ID/)).toBeNull();
    // The clipboard policy is stated before any copy happens.
    expect(screen.getByText(/the clipboard clears itself after 30 seconds/)).toBeDefined();
    expect(screen.getByText('Open Typvia · Fingerprint')).toBeDefined();
    expect(screen.getByText('Copy once — clears in 30s')).toBeDefined();
    expect(screen.getByText('Come back and paste')).toBeDefined();
  });

  it('copies once on Android and renders the countdown from the returned delay', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Prod bastion token')]);
    // A non-default delay proves the UI reads the wire value, not a constant.
    ipc.vaultCopySecret.mockResolvedValue(5_000);
    render(<VaultPage />);
    await screen.findByText('Prod bastion token');

    fireEvent.click(screen.getByRole('button', { name: 'Copy 30s' }));

    expect(await screen.findByText('clears in 5s')).toBeDefined();
    expect(ipc.vaultCopySecret).toHaveBeenCalledWith('s-1');
    expect(ipc.vaultCopySecret).toHaveBeenCalledTimes(1);
    // Copying never implies a reveal: the value stays masked.
    expect(ipc.vaultReveal).not.toHaveBeenCalled();
  });

  it('keeps the Android row intact on a copy failure and says the secret is still safe', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Prod bastion token')]);
    ipc.vaultCopySecret.mockRejectedValue(new Error('locked'));
    render(<VaultPage />);
    await screen.findByText('Prod bastion token');

    fireEvent.click(screen.getByRole('button', { name: 'Copy 30s' }));

    const alert = await screen.findByRole('alert');
    // Failure copy leads with what is still fine.
    expect(alert.textContent).toContain('Your secret is still encrypted');
    expect(screen.getByRole('button', { name: 'Copy 30s' })).toBeDefined();
    expect(screen.queryByRole('status')).toBeNull();
  });
});
