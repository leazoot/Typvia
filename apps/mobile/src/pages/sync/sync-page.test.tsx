// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MobileSyncPage } from './sync-page';

const ipc = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncDevices: vi.fn(),
  syncConflicts: vi.fn(),
  syncNow: vi.fn(),
  syncRevokeDevice: vi.fn(),
  syncEnable: vi.fn(),
  syncDisable: vi.fn(),
  syncResume: vi.fn(),
  syncRotateKey: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

function status(overrides: Partial<SharedModule.SyncStatus> = {}): SharedModule.SyncStatus {
  return {
    available: true,
    configured: true,
    enabled: true,
    serverUrl: 'https://sync.example.com',
    accountId: 'acct-1',
    deviceId: 'd-0',
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

function device(overrides: Partial<SharedModule.SyncDevice> = {}): SharedModule.SyncDevice {
  return {
    deviceId: 'd-0',
    name: 'This iPhone',
    platform: 'ios',
    createdAt: 1,
    revokedAt: null,
    verified: true,
    isThisDevice: true,
    isRoot: true,
    ...overrides,
  };
}

const noop = () => undefined;

function renderPage() {
  return render(
    <MobileSyncPage
      onBack={noop}
      onPair={noop}
      onConflicts={noop}
      onRecovery={noop}
      onRecover={noop}
    />,
  );
}

beforeEach(() => {
  ipc.syncStatus.mockResolvedValue(status());
  ipc.syncConflicts.mockResolvedValue([]);
  ipc.syncDevices.mockResolvedValue([device()]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile Sync screen', () => {
  it('shows text lines while the status is in flight — never a spinner', () => {
    ipc.syncStatus.mockReturnValue(new Promise(() => {}));
    const { container } = renderPage();

    expect(container.querySelectorAll('.tv-mset-skeleton').length).toBeGreaterThan(0);
    expect(container.querySelectorAll('svg, img')).toHaveLength(0);
  });

  it('offers the three first-use choices before an account exists', async () => {
    ipc.syncStatus.mockResolvedValue(status({ configured: false }));

    renderPage();

    expect(await screen.findByText('Your text lives on this device only.')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Start an account here' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Join from another device' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Recover with a recovery code' })).toBeDefined();
  });

  it('says sync cannot be set up here rather than offering a dead button', async () => {
    ipc.syncStatus.mockResolvedValue(status({ available: false, configured: false }));

    renderPage();

    expect(await screen.findByText('Sync needs secure key storage.')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Start an account here' })).toBeNull();
  });

  it('lists the account devices and runs a round on demand', async () => {
    ipc.syncDevices.mockResolvedValue([
      device(),
      device({ deviceId: 'd-1', name: 'This Mac', isThisDevice: false, isRoot: false }),
    ]);
    ipc.syncNow.mockResolvedValue({
      pushed: 2,
      applied: 3,
      merged: 0,
      conflictCopies: 0,
      parked: 0,
      skipped: 0,
      pendingBacklog: 0,
      at: 1,
    });

    renderPage();

    expect(await screen.findByText('Your text lives on 2 devices.')).toBeDefined();
    expect(screen.getByText('This Mac')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Sync now' }));

    expect(await screen.findByText('Sent 2, received 3.')).toBeDefined();
  });

  it('reads an unreachable server as offline, not as a failure', async () => {
    const { IpcError } = await vi.importActual<typeof SharedModule>('@typvia/shared');
    ipc.syncNow.mockRejectedValue(new IpcError('unavailable', 'the server could not be reached'));

    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Sync now' }));

    expect(await screen.findByText(/still here and still queued/)).toBeDefined();
  });

  it('names the device before revoking it and only acts on confirmation', async () => {
    ipc.syncDevices.mockResolvedValue([
      device(),
      device({
        deviceId: 'd-1',
        name: 'Pixel 8',
        platform: 'android',
        isThisDevice: false,
        isRoot: false,
      }),
    ]);
    ipc.syncRevokeDevice.mockResolvedValue(2);

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Revoke' }));
    expect(screen.getByText(/Revoke Pixel 8\?/)).toBeDefined();
    expect(ipc.syncRevokeDevice).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Revoke Pixel 8' }));
    await waitFor(() => {
      expect(ipc.syncRevokeDevice).toHaveBeenCalledWith('d-1');
    });
  });

  it('turns sync off through the toggle and keeps the product usable', async () => {
    ipc.syncDisable.mockResolvedValue(status({ enabled: false }));

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Turn sync off' }));
    await waitFor(() => {
      expect(ipc.syncDisable).toHaveBeenCalled();
    });
  });

  it('surfaces a pending decision as a warning row rather than an error', async () => {
    ipc.syncConflicts.mockResolvedValue([
      {
        source: { id: 's-1', title: 'Weekly' },
        copy: { id: 's-2', title: 'Weekly' },
        sensitive: false,
      } as unknown as SharedModule.ConflictPair,
    ]);

    renderPage();

    expect(await screen.findByText('1 snippet changed in two places')).toBeDefined();
    expect(screen.getByText(/Nothing is deleted/)).toBeDefined();
  });
});
