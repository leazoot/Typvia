// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';

const ipc = vi.hoisted(() => ({
  mobileBootstrap: vi.fn(),
  listSnippetPage: vi.fn(),
  countSnippets: vi.fn(),
  searchLibrary: vi.fn(),
  createSnippet: vi.fn(),
  updateSnippet: vi.fn(),
  searchSnippetsAll: vi.fn(),
  trashSnippet: vi.fn(),
  listFolderChildren: vi.fn(),
  historyList: vi.fn(),
  snapshotRefresh: vi.fn(),
  shareInboxIngest: vi.fn(),
  onboardingStatus: vi.fn(),
  onboardingComplete: vi.fn(),
  openKeyboardSettings: vi.fn(),
  vaultStatus: vi.fn(),
  vaultInitialize: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockBiometric: vi.fn(),
  vaultEnableBiometric: vi.fn(),
  vaultLock: vi.fn(),
  vaultList: vi.fn(),
  vaultReveal: vi.fn(),
  vaultCreateSecret: vi.fn(),
  copySnippet: vi.fn(),
  syncStatus: vi.fn(),
  syncConflicts: vi.fn(),
  syncDevices: vi.fn(),
  syncNow: vi.fn(),
  getSnippet: vi.fn(),
}));

// Deep-link plugin mock: defaults mirror a launch without any
// deep link — no cold-start URL, a live listener that never fires.
const deepLink = vi.hoisted(() => ({
  getCurrent: vi.fn(async (): Promise<string[] | null> => null),
  onOpenUrl: vi.fn(async () => () => undefined),
}));

vi.mock('@tauri-apps/plugin-deep-link', () => ({
  getCurrent: deepLink.getCurrent,
  onOpenUrl: deepLink.onOpenUrl,
}));

/** The sync status the settings surfaces read; overridable per test. */
function syncStatus(overrides: Partial<SharedModule.SyncStatus> = {}): SharedModule.SyncStatus {
  return {
    available: true,
    configured: true,
    enabled: true,
    serverUrl: 'https://sync.example.com',
    accountId: 'acct-1',
    deviceId: 'd-1',
    deviceName: 'This iPhone',
    keyGeneration: 1,
    pendingBacklog: 0,
    conflictCount: 0,
    lastSyncAt: null,
    vaultReady: false,
    vaultUnlocked: false,
    recoveryExportedAt: null,
    recoveryCatchupPending: false,
    transportKind: 'server',
    ...overrides,
  };
}

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

const SAVED_SNIPPET = {
  id: 's-9',
  title: 'Tail logs',
  body: 'kubectl logs',
  snippetType: 'command',
  securityLevel: 'normal',
  description: null,
  folderId: null,
  trigger: ';klogs',
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

function bootReady(snippetTotal: number) {
  ipc.mobileBootstrap.mockResolvedValue({ schemaVersion: 9, snippetTotal });
  ipc.listSnippetPage.mockResolvedValue([]);
  ipc.countSnippets.mockResolvedValue(snippetTotal);
  ipc.searchLibrary.mockResolvedValue([]);
  ipc.searchSnippetsAll.mockResolvedValue([]);
  ipc.syncStatus.mockResolvedValue(syncStatus({ enabled: false }));
  ipc.listFolderChildren.mockResolvedValue([]);
  ipc.historyList.mockResolvedValue({ current: 1, entries: [] });
  ipc.onboardingStatus.mockResolvedValue({ completed: true });
  ipc.onboardingComplete.mockResolvedValue(undefined);
  ipc.shareInboxIngest.mockResolvedValue(0);
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile app shell', () => {
  it('shows the loading state while the boot handshake is in flight', async () => {
    ipc.onboardingStatus.mockResolvedValue({ completed: true });
    ipc.mobileBootstrap.mockReturnValue(new Promise(() => {}));
    render(<App />);
    const status = await screen.findByRole('status');
    expect(status.textContent).toContain('Opening your library');
  });

  it('says the data is untouched when the core cannot be opened', async () => {
    ipc.onboardingStatus.mockResolvedValue({ completed: true });
    ipc.mobileBootstrap.mockRejectedValue(new Error('internal storage error'));
    render(<App />);

    const status = await screen.findByText(/Your data is untouched on this device/);
    expect(status.textContent).toContain('internal storage error');
    // No navigation is offered over a library that could not open.
    expect(screen.queryByRole('navigation')).toBeNull();
  });

  it('boots into Home with the four-icon tab bar and the create square', async () => {
    bootReady(2);
    render(<App />);

    expect(await screen.findByRole('button', { name: 'Search snippets' })).toBeDefined();
    const nav = screen.getByRole('navigation', { name: 'Main' });
    // One line icon per tab; the create square draws its cross in CSS.
    expect(nav.querySelectorAll('svg')).toHaveLength(4);
    expect(screen.getByRole('button', { name: 'New snippet' })).toBeDefined();
  });

  it('switches between Home and Library without a page transition', async () => {
    bootReady(2);
    render(<App />);
    await screen.findByText('Typvia');

    fireEvent.click(screen.getByRole('button', { name: /Library/ }));
    expect(await screen.findByRole('group', { name: 'Filter by type' })).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: /Home/ }));
    expect(await screen.findByText('Typvia')).toBeDefined();
  });

  it('opens Settings from its tab with the Sync row as its only entry', async () => {
    bootReady(2);
    ipc.syncStatus.mockResolvedValue(syncStatus());
    render(<App />);
    await screen.findByText('Typvia');

    fireEvent.click(screen.getByRole('button', { name: /Settings/ }));

    // The settings root row: a label, a state reading and a chevron into the
    // pushed screen. Nothing else is faked while its groups are unbuilt.
    const row = await screen.findByRole('button', { name: /Sync/ });
    expect(row.textContent).toContain('on');
  });

  it('reaches the pushed Sync screen and comes back to Settings', async () => {
    bootReady(2);
    ipc.syncStatus.mockResolvedValue(syncStatus());
    ipc.syncConflicts.mockResolvedValue([]);
    ipc.syncDevices.mockResolvedValue([
      {
        deviceId: 'd-1',
        name: 'This iPhone',
        platform: 'ios',
        createdAt: 1,
        revokedAt: null,
        verified: true,
        isThisDevice: true,
        isRoot: true,
      },
    ]);
    render(<App />);
    await screen.findByText('Typvia');
    fireEvent.click(screen.getByRole('button', { name: /Settings/ }));
    fireEvent.click(await screen.findByRole('button', { name: /Sync/ }));

    expect(await screen.findByText('Your text lives on one device.')).toBeDefined();
    // The back bar names the parent screen, never a chevron glyph. The tab
    // bar underneath also says "Settings", so pick the back bar itself.
    const back = screen
      .getAllByRole('button', { name: 'Settings' })
      .find((button) => button.classList.contains('tv-mset-back'));
    expect(back).toBeDefined();
    fireEvent.click(back as HTMLElement);
    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeDefined();
  });

  it('opens the Vault page from its tab', async () => {
    bootReady(2);
    ipc.vaultStatus.mockResolvedValue({
      initialized: false,
      unlocked: false,
      unlockedAt: null,
      lastActivityAt: null,
      idleTimeoutMs: 300_000,
    });
    render(<App />);
    await screen.findByText('Typvia');

    fireEvent.click(screen.getByRole('button', { name: /Vault/ }));
    // An uninitialized vault lands on the honest first-use setup page.
    expect(await screen.findByText('Set up the vault')).toBeDefined();
  });

  it('opens the Search screen from the Home line and returns on Cancel', async () => {
    bootReady(2);
    render(<App />);
    await screen.findByText('Typvia');

    fireEvent.click(screen.getByRole('button', { name: 'Search snippets' }));
    // The full-screen Search: a live input plus Cancel, no tab bar.
    fireEvent.change(screen.getByLabelText('Search snippets'), { target: { value: 'd' } });
    expect(ipc.searchSnippetsAll).toHaveBeenCalledWith('d', expect.any(Number));

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(await screen.findByText('Typvia')).toBeDefined();
  });

  it('opens the create page from the centre caret and returns on cancel', async () => {
    bootReady(2);
    render(<App />);
    await screen.findByText('Typvia');

    fireEvent.click(screen.getByRole('button', { name: 'New snippet' }));
    // The create sheet floats over the dimmed shell — the shell
    // and its tab bar stay mounted underneath.
    expect(screen.getByRole('dialog', { name: 'New snippet' })).toBeDefined();
    expect(screen.getByRole('navigation')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(await screen.findByText('Typvia')).toBeDefined();
    expect(ipc.createSnippet).not.toHaveBeenCalled();
  });

  it('returns after a save with the Saved toast and freshly fetched lists', async () => {
    bootReady(2);
    ipc.createSnippet.mockResolvedValue(SAVED_SNIPPET);
    render(<App />);
    await screen.findByText('Typvia');
    const recentFetches = ipc.listSnippetPage.mock.calls.length;

    fireEvent.click(screen.getByRole('button', { name: 'New snippet' }));
    fireEvent.change(screen.getByLabelText('Snippet content'), {
      target: { value: 'kubectl logs' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    // Back on Home, with the trigger named in the assertive toast.
    const toast = await screen.findByText('Saved · ;klogs');
    expect(toast.closest('[aria-live="assertive"]')).not.toBeNull();
    // The list refresh signal: the page remounts and refetches, and the
    // boot totals are re-read so Home's count stays honest.
    expect(ipc.listSnippetPage.mock.calls.length).toBeGreaterThan(recentFetches);
    expect(ipc.mobileBootstrap).toHaveBeenCalledTimes(2);
  });

  it('renders nothing while the first-run status is unknown', () => {
    ipc.onboardingStatus.mockReturnValue(new Promise(() => {}));
    ipc.mobileBootstrap.mockReturnValue(new Promise(() => {}));
    const { container } = render(<App />);
    // The flow decision is a cut, not a flash of shell or onboarding.
    expect(container.innerHTML).toBe('');
  });

  it('shows onboarding on first run and hands over to the shell when done', async () => {
    bootReady(0);
    ipc.onboardingStatus.mockResolvedValue({ completed: false });
    render(<App />);

    expect(await screen.findByText('Step 1 of 6')).toBeDefined();
    // No shell underneath: onboarding is a full takeover.
    expect(screen.queryByRole('navigation')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    expect(await screen.findByText('Typvia')).toBeDefined();
    expect(ipc.onboardingComplete).toHaveBeenCalled();
  });

  it('parks a deep link behind first-run onboarding and honors it on completion', async () => {
    // Onboarding stays in charge of first run — a deep link never skips it —
    // but the link is parked, not dropped, and lands right after the
    // handover.
    bootReady(1);
    ipc.onboardingStatus.mockResolvedValue({ completed: false });
    const linked = { ...SAVED_SNIPPET, id: '4c8e2c1a-9a1b-4f5e-8a2d-1b2c3d4e5f60' };
    ipc.getSnippet.mockResolvedValueOnce(linked);
    deepLink.getCurrent.mockResolvedValueOnce([`typvia://snippet/${linked.id}`]);
    render(<App />);

    expect(await screen.findByText('Step 1 of 6')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Skip setup' }));
    // The link lands on the snippet detail screen.
    expect(await screen.findByRole('heading', { name: 'Tail logs' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Copy' })).toBeDefined();
  });

  it('fails open into the shell when the first-run status cannot be read', async () => {
    bootReady(2);
    ipc.onboardingStatus.mockRejectedValue(new Error('io'));
    render(<App />);
    // The app staying usable matters more than re-offering setup.
    expect(await screen.findByText('Typvia')).toBeDefined();
    expect(screen.queryByText('Step 1 of 6')).toBeNull();
  });

  it('drains the share inbox once the shell is ready and toasts the count', async () => {
    bootReady(2);
    ipc.shareInboxIngest.mockResolvedValue(2);
    render(<App />);

    const toast = await screen.findByText('Saved from share · 2');
    expect(toast.closest('[aria-live="assertive"]')).not.toBeNull();
    expect(ipc.shareInboxIngest).toHaveBeenCalledTimes(1);
    // The boot totals are re-read so Home's count stays honest.
    expect(ipc.mobileBootstrap).toHaveBeenCalledTimes(2);
  });

  it('shows no share toast when the inbox drain finds nothing', async () => {
    bootReady(2);
    render(<App />);
    await screen.findByText('Typvia');

    await act(async () => {});
    expect(ipc.shareInboxIngest).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/Saved from share/)).toBeNull();
  });

  it('schedules a debounced keyboard snapshot refresh after a save', async () => {
    vi.useFakeTimers();
    try {
      bootReady(2);
      ipc.createSnippet.mockResolvedValue(SAVED_SNIPPET);
      ipc.snapshotRefresh.mockResolvedValue(undefined);
      render(<App />);
      // Flush the boot handshake without advancing past the debounce window
      // (findBy* would spin: RTL cannot advance vitest's fake timers).
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });

      fireEvent.click(screen.getByRole('button', { name: 'New snippet' }));
      fireEvent.change(screen.getByLabelText('Snippet content'), {
        target: { value: 'kubectl logs' },
      });
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      screen.getByText('Saved · ;klogs');

      // Trailing debounce (desktop notifyMutation parity): nothing is
      // written during the burst window, one refresh after it elapses.
      expect(ipc.snapshotRefresh).not.toHaveBeenCalled();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(900);
      });
      expect(ipc.snapshotRefresh).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
