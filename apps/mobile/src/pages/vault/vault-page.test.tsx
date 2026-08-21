// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
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

function secret(id: string, title: string, folderId: string | null = null) {
  return {
    id,
    title,
    body: null,
    snippetType: 'sensitive',
    securityLevel: 'sensitive',
    description: null,
    folderId,
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

function folder(id: string, name: string) {
  return { id, parentId: null, name, sortOrder: 0, createdAt: 1, updatedAt: 1 };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile vault page', () => {
  it('shows the first-use setup when the vault is not initialized', async () => {
    ipc.vaultStatus.mockResolvedValue(status({ initialized: false }));
    render(<VaultPage />);

    expect(await screen.findByText('Set up the vault')).toBeDefined();
    expect(screen.getByLabelText('Master password')).toBeDefined();
    // Honest no-recovery copy before any error appears.
    expect(screen.getByText(/There is no recovery if you forget it/)).toBeDefined();
  });

  it('creates the vault from setup and lands on the unlocked list', async () => {
    ipc.vaultStatus.mockResolvedValue(status({ initialized: false }));
    ipc.vaultInitialize.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    ipc.listFolderChildren.mockResolvedValue([]);
    render(<VaultPage />);
    await screen.findByText('Set up the vault');

    fireEvent.change(screen.getByLabelText('Master password'), {
      target: { value: 'correct horse' },
    });
    fireEvent.change(screen.getByLabelText('Confirm master password'), {
      target: { value: 'correct horse' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create vault' }));

    expect(await screen.findByText(/Nothing in the vault yet/)).toBeDefined();
    expect(ipc.vaultInitialize).toHaveBeenCalledWith('correct horse');
  });

  it('rejects a short or mismatched password without calling the host', async () => {
    ipc.vaultStatus.mockResolvedValue(status({ initialized: false }));
    render(<VaultPage />);
    await screen.findByText('Set up the vault');

    fireEvent.change(screen.getByLabelText('Master password'), { target: { value: 'short' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create vault' }));
    expect(await screen.findByRole('alert')).toBeDefined();
    expect(ipc.vaultInitialize).not.toHaveBeenCalled();
  });

  it('shows the locked page with both unlock paths and no vault data', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    render(<VaultPage />);

    expect(await screen.findByText('Protected on this device')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Use Face ID' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Enter passcode' })).toBeDefined();
    // While locked nothing is fetched — not even metadata; the backdrop
    // lines are generic placeholders, not blurred real rows.
    expect(ipc.vaultList).not.toHaveBeenCalled();
    expect(ipc.vaultReveal).not.toHaveBeenCalled();
  });

  it('resets the vault from the lost-password flow with the real count', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    ipc.vaultResetPreview.mockResolvedValue(2);
    ipc.vaultReset.mockResolvedValue(status({ initialized: false }));
    render(<VaultPage />);
    await screen.findByText('Protected on this device');

    fireEvent.click(screen.getByRole('button', { name: 'Enter passcode' }));
    fireEvent.click(screen.getByRole('button', { name: 'Forgot the master password?' }));
    // Two steps: the first names the action, the confirm names the real count.
    fireEvent.click(await screen.findByRole('button', { name: 'Reset the vault' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Destroy 2 secret snippets forever' }),
    );

    // The fresh uninitialized status drops straight into first-use setup.
    expect(await screen.findByText('Set up the vault')).toBeDefined();
    expect(ipc.vaultReset).toHaveBeenCalledTimes(1);
  });

  it('unlocks with Face ID and shows the LOCKS IN countdown', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Prod read replica')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultUnlockBiometric.mockResolvedValue(unlockedStatus());
    render(<VaultPage />);
    await screen.findByText('Protected on this device');

    fireEvent.click(screen.getByRole('button', { name: 'Use Face ID' }));

    expect(await screen.findByText('Prod read replica')).toBeDefined();
    // The auto-lock reading, computed from the host's idle timeout.
    expect(screen.getByText(/LOCKS IN [0-5]:\d\d/)).toBeDefined();
  });

  it('collapses a Face ID failure to the generic copy and the password path', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    ipc.vaultUnlockBiometric.mockRejectedValue(new Error('keychain OSStatus -25293'));
    render(<VaultPage />);
    await screen.findByText('Protected on this device');

    fireEvent.click(screen.getByRole('button', { name: 'Use Face ID' }));

    const alert = await screen.findByRole('alert');
    // Static failure copy: says what is still good, never why it failed.
    expect(alert.textContent).toContain('Still encrypted and safe');
    expect(screen.queryByText(/OSStatus/)).toBeNull();
    expect(screen.getByLabelText('Master password')).toBeDefined();
  });

  it('collapses a wrong master password to the same generic copy', async () => {
    ipc.vaultStatus.mockResolvedValue(status());
    ipc.vaultUnlockPassword.mockRejectedValue(new Error('unlock failed'));
    render(<VaultPage />);
    await screen.findByText('Protected on this device');

    fireEvent.click(screen.getByRole('button', { name: 'Enter passcode' }));
    fireEvent.change(screen.getByLabelText('Master password'), { target: { value: 'guess' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }));

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Still encrypted and safe');
    // The vault stays locked: the password form is still there to retry.
    expect(screen.getByRole('button', { name: 'Unlock' })).toBeDefined();
  });

  it('reveals one secret while held and hides it again on release', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Stripe test key')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultReveal.mockResolvedValue('sk_test_51Nf');
    render(<VaultPage />);
    await screen.findByText('Stripe test key');

    // The list never carries plaintext; reveal is a per-row, on-demand call.
    expect(ipc.vaultReveal).not.toHaveBeenCalled();
    expect(screen.queryByText('sk_test_51Nf')).toBeNull();

    const hold = screen.getByRole('button', { name: 'Hold to reveal Stripe test key' });
    fireEvent.pointerDown(hold);
    expect(await screen.findByText('sk_test_51Nf')).toBeDefined();
    expect(ipc.vaultReveal).toHaveBeenCalledWith('s-1');

    // Release resets to the mask — nothing lingers on screen.
    fireEvent.pointerUp(hold);
    expect(screen.queryByText('sk_test_51Nf')).toBeNull();
  });

  it('never shows plaintext when the press ends before the decrypt lands', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Stripe test key')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    let resolveReveal: (text: string) => void = () => undefined;
    ipc.vaultReveal.mockImplementation(
      () =>
        new Promise<string>((resolve) => {
          resolveReveal = resolve;
        }),
    );
    render(<VaultPage />);
    await screen.findByText('Stripe test key');

    const hold = screen.getByRole('button', { name: 'Hold to reveal Stripe test key' });
    fireEvent.pointerDown(hold);
    fireEvent.pointerUp(hold);
    await act(async () => {
      resolveReveal('sk_test_late');
      await Promise.resolve();
    });

    // The decrypt landed after the finger lifted: it is dropped, not shown.
    expect(screen.queryByText('sk_test_late')).toBeNull();
  });

  it('renders the flat SECRET meta line with the resolved folder name', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([
      secret('s-1', 'Stripe live key', 'f-1'),
      secret('s-2', 'Wi-Fi — Home'),
    ]);
    ipc.listFolderChildren.mockResolvedValue([folder('f-1', 'Work')]);
    render(<VaultPage />);
    await screen.findByText('Stripe live key');

    // One flat kind — no sub-types; the folder joins with a middot when known.
    expect(await screen.findByText('SECRET · Work')).toBeDefined();
    expect(screen.getByText('SECRET')).toBeDefined();
    // The reveal contract is stated on the page itself.
    expect(screen.getByText('Hold to reveal · nothing is copied to the clipboard')).toBeDefined();
  });

  it('copies once and renders the countdown from the returned delay', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Stripe test key')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    // A non-default delay proves the UI reads the wire value, not a constant.
    ipc.vaultCopySecret.mockResolvedValue(5_000);
    render(<VaultPage />);
    await screen.findByText('Stripe test key');

    fireEvent.click(screen.getByRole('button', { name: 'Copy 30s' }));

    expect(await screen.findByText('clears in 5s')).toBeDefined();
    expect(ipc.vaultCopySecret).toHaveBeenCalledWith('s-1');
    expect(ipc.vaultCopySecret).toHaveBeenCalledTimes(1);
    // Copying never implies a reveal: the value stays masked, no decrypt call.
    expect(ipc.vaultReveal).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Hold to reveal Stripe test key' })).toBeDefined();
  });

  it('ends the countdown quietly once the OS-enforced clear elapses', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Stripe test key')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultCopySecret.mockResolvedValue(2_000);
    render(<VaultPage />);
    await screen.findByText('Stripe test key');

    vi.useFakeTimers();
    try {
      fireEvent.click(screen.getByRole('button', { name: 'Copy 30s' }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(screen.getByText('clears in 2s')).toBeDefined();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3_000);
      });
      // The OS enforced the clear — nothing special, the action returns.
      expect(screen.queryByRole('status')).toBeNull();
      expect(screen.getByRole('button', { name: 'Copy 30s' })).toBeDefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps the row intact on a copy failure and says the secret is still safe', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'Stripe test key')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultCopySecret.mockRejectedValue(new Error('locked'));
    render(<VaultPage />);
    await screen.findByText('Stripe test key');

    fireEvent.click(screen.getByRole('button', { name: 'Copy 30s' }));

    const alert = await screen.findByRole('alert');
    // Failure copy leads with what is still fine.
    expect(alert.textContent).toContain('Your secret is still encrypted');
    // The row is intact: both actions remain, no countdown started.
    expect(screen.getByRole('button', { name: 'Copy 30s' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Hold to reveal Stripe test key' })).toBeDefined();
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('explains the secure-field round trip in three honest steps', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    ipc.listFolderChildren.mockResolvedValue([]);
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    expect(screen.getByText('Secure field — the honest downgrade, in three steps')).toBeDefined();
    // The platform is named as the cause; the clipboard policy is stated.
    expect(screen.getByText(/iOS won’t let any third-party keyboard type here/)).toBeDefined();
    expect(screen.getByText(/the clipboard clears itself after 30 seconds/)).toBeDefined();
    expect(screen.getByText('Open Typvia · Face ID')).toBeDefined();
    expect(screen.getByText('Copy once — clears in 30s')).toBeDefined();
    expect(screen.getByText('Come back and paste')).toBeDefined();
  });

  it('creates a secret as sensitive and refreshes the list', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValueOnce([]).mockResolvedValue([secret('s-9', 'Bank card')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultCreateSecret.mockResolvedValue(secret('s-9', 'Bank card'));
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    fireEvent.click(screen.getByRole('button', { name: 'New secret' }));
    fireEvent.change(screen.getByLabelText('Secret title'), { target: { value: 'Bank card' } });
    fireEvent.change(screen.getByLabelText('Secret value'), {
      target: { value: '6222 0000 0000 4418' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save secret' }));

    expect(await screen.findByText('Bank card')).toBeDefined();
    expect(ipc.vaultCreateSecret).toHaveBeenCalledWith(
      expect.objectContaining({
        title: 'Bank card',
        body: '6222 0000 0000 4418',
        snippetType: 'sensitive',
      }),
    );
  });

  it('says the secrets are still safe when the unlocked list fails to load', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockRejectedValue(new Error('io'));
    ipc.listFolderChildren.mockResolvedValue([]);
    render(<VaultPage />);

    const notice = await screen.findByText(/Your secrets are safe and encrypted on this device/);
    expect(notice).toBeDefined();
  });

  it('locks immediately from Lock now and returns to the locked page', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([secret('s-1', 'A')]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultLock.mockResolvedValue(status());
    render(<VaultPage />);
    await screen.findByText('A');

    fireEvent.click(screen.getByRole('button', { name: 'Lock now' }));

    expect(await screen.findByText('Protected on this device')).toBeDefined();
    expect(ipc.vaultLock).toHaveBeenCalledTimes(1);
  });

  it('offers Face ID enrollment and keeps the password path on failure', async () => {
    ipc.vaultStatus.mockResolvedValue(unlockedStatus());
    ipc.vaultList.mockResolvedValue([]);
    ipc.listFolderChildren.mockResolvedValue([]);
    ipc.vaultEnableBiometric.mockRejectedValue(new Error('unavailable'));
    render(<VaultPage />);
    await screen.findByText(/Nothing in the vault yet/);

    fireEvent.click(screen.getByRole('button', { name: 'Enable Face ID for faster unlock' }));
    expect(await screen.findByText(/the master password still works/)).toBeDefined();
  });

  it('reports a failed status read without pretending anything was lost', async () => {
    ipc.vaultStatus.mockRejectedValue(new Error('io'));
    render(<VaultPage />);

    expect(await screen.findByText('Your secrets are safe on this device.')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeDefined();
  });
});
