// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { ConflictPair, Snippet, SyncDevice, SyncStatus } from '@typvia/shared';
import type * as SharedModule from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SyncPage } from './sync-page';

const mocks = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncDevices: vi.fn(),
  syncConflicts: vi.fn(),
  syncNow: vi.fn(),
  syncRevokeDevice: vi.fn(),
  syncRotateKey: vi.fn(),
  syncResume: vi.fn(),
  syncEnable: vi.fn(),
  syncEnableWebdav: vi.fn(),
  webdavCredentials: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...mocks };
});

function status(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    available: true,
    configured: true,
    enabled: true,
    serverUrl: 'https://sync.example.com',
    accountId: 'account-1',
    deviceId: 'device-a',
    deviceName: 'MacBook Pro',
    keyGeneration: 1,
    pendingBacklog: 0,
    conflictCount: 0,
    lastSyncAt: null,
    vaultReady: true,
    vaultUnlocked: true,
    recoveryExportedAt: null,
    recoveryCatchupPending: false,
    transportKind: 'server',
    ...overrides,
  };
}

function device(overrides: Partial<SyncDevice> = {}): SyncDevice {
  return {
    deviceId: 'device-a',
    name: 'MacBook Pro',
    platform: 'macos',
    createdAt: 1,
    revokedAt: null,
    verified: true,
    isThisDevice: true,
    isRoot: true,
    ...overrides,
  };
}

function snippet(id: string, title: string, body: string | null): Snippet {
  return {
    id,
    title,
    body,
    snippetType: 'text',
    securityLevel: body === null ? 'sensitive' : 'normal',
    description: null,
    folderId: null,
    trigger: ';weekly',
    triggerMode: null,
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: 2,
    lastUsedAt: null,
    usageCount: 0,
    version: 1,
    deletedAt: null,
  };
}

function pair(): ConflictPair {
  return {
    source: snippet('source-1', 'Weekly update', 'the version in use'),
    copy: snippet('copy-1', 'Weekly update (conflict on Pixel 8)', 'the version that arrived'),
    sensitive: false,
  };
}

function renderPage() {
  return render(
    <MemoryRouter initialEntries={['/sync']}>
      <SyncPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.syncStatus.mockResolvedValue(status());
  mocks.syncDevices.mockResolvedValue([device()]);
  mocks.syncConflicts.mockResolvedValue([]);
  // The core composes the credential string; the page only carries it.
  mocks.webdavCredentials.mockResolvedValue('basic:FAKE_user:FAKE_dav_pw');
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('Sync & devices', () => {
  it('shows a skeleton route before the first status arrives', () => {
    mocks.syncStatus.mockReturnValue(new Promise(() => undefined));
    const { container } = renderPage();

    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    expect(container.querySelectorAll('.tv-sync-skeleton-line').length).toBeGreaterThan(0);
  });

  it('offers three real next actions when no account is set up yet', async () => {
    mocks.syncStatus.mockResolvedValue(status({ configured: false, enabled: false }));
    renderPage();

    expect(await screen.findByRole('button', { name: 'Start an account here' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Join from another device' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Recover with a recovery code' })).toBeTruthy();
  });

  it('founds a WebDAV account with credentials that never touch the library', async () => {
    mocks.syncStatus.mockResolvedValue(status({ configured: false, enabled: false }));
    mocks.syncEnableWebdav.mockResolvedValue(status());
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'My WebDAV storage' }));
    fireEvent.change(screen.getByLabelText('WebDAV address'), {
      target: { value: 'https://dav.example.com/typvia' },
    });
    fireEvent.change(screen.getByPlaceholderText('Username'), {
      target: { value: 'FAKE_user' },
    });
    fireEvent.change(screen.getByLabelText('WebDAV password'), {
      target: { value: 'FAKE_dav_pw' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start an account here' }));

    // The page hands on whatever the core composed. It is deliberately not
    // asserted to be `basic:user:pass` here: that shape belongs beside the
    // parser that reads it back, and a screen that restated it would keep
    // passing this test after the real one changed.
    await waitFor(() =>
      expect(mocks.webdavCredentials).toHaveBeenCalledWith('FAKE_user', 'FAKE_dav_pw'),
    );
    await waitFor(() =>
      expect(mocks.syncEnableWebdav).toHaveBeenCalledWith(
        'https://dav.example.com/typvia',
        'basic:FAKE_user:FAKE_dav_pw',
      ),
    );
  });

  it('says what still works when the device has no secure key storage', async () => {
    mocks.syncStatus.mockResolvedValue(status({ available: false, configured: false }));
    renderPage();

    expect(await screen.findByText(/Sync needs secure key storage/)).toBeTruthy();
    expect(screen.getByText(/Everything else works exactly as before/)).toBeTruthy();
  });

  it('draws one route row per device with this device marked', async () => {
    mocks.syncDevices.mockResolvedValue([
      device(),
      device({
        deviceId: 'device-b',
        name: 'Pixel 8',
        platform: 'android',
        isThisDevice: false,
        isRoot: false,
      }),
    ]);
    const { container } = renderPage();

    expect(await screen.findByText('Your text lives on 2 devices.')).toBeTruthy();
    expect(screen.getByText('Pixel 8')).toBeTruthy();
    expect(container.querySelectorAll('.tv-sync-node-self')).toHaveLength(1);
  });

  it('names the pending recovery catch-up and that it resumes on its own', async () => {
    mocks.syncStatus.mockResolvedValue(status({ recoveryCatchupPending: true }));
    renderPage();

    expect(await screen.findByText(/history catch-up finishes in the background/)).toBeTruthy();
  });

  it('draws the line dashed and says offline when the server cannot be reached', async () => {
    const { IpcError } = await vi.importActual<typeof SharedModule>('@typvia/shared');
    mocks.syncDevices.mockRejectedValue(
      new IpcError('unavailable', 'the server could not be reached'),
    );
    const { container } = renderPage();

    await waitFor(() => {
      expect(container.querySelector('.tv-sync-line-offline')).not.toBeNull();
    });
  });

  it('marks a revoked device with a struck node instead of hiding it', async () => {
    mocks.syncDevices.mockResolvedValue([
      device(),
      device({
        deviceId: 'device-b',
        name: 'Old laptop',
        isThisDevice: false,
        isRoot: false,
        revokedAt: 5,
      }),
    ]);
    const { container } = renderPage();

    expect(await screen.findByText('Old laptop')).toBeTruthy();
    expect(container.querySelectorAll('.tv-sync-node-revoked')).toHaveLength(1);
  });

  it('shows the conflict marker and a route into the decision', async () => {
    mocks.syncConflicts.mockResolvedValue([pair()]);
    const { container } = renderPage();

    expect(await screen.findByText('One snippet changed in two places')).toBeTruthy();
    expect(screen.getByText('1 conflict to resolve')).toBeTruthy();
    expect(container.querySelector('.tv-sync-node-conflict')).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Decide now' })).toBeTruthy();
  });

  it('reports what a round did and stays calm when it could not reach the server', async () => {
    const { IpcError } = await vi.importActual<typeof SharedModule>('@typvia/shared');
    mocks.syncNow.mockRejectedValue(new IpcError('unavailable', 'the server could not be reached'));
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Sync now' }));

    expect(
      await screen.findByText(/Everything you saved is still here and still queued/),
    ).toBeTruthy();
  });

  it('reads offline, never "syncing now", once a round could not connect', async () => {
    const { IpcError } = await vi.importActual<typeof SharedModule>('@typvia/shared');
    const down = new IpcError('unavailable', 'the server could not be reached');
    mocks.syncNow.mockRejectedValue(down);
    mocks.syncDevices.mockRejectedValue(down);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Sync now' }));

    expect(await screen.findByText('offline')).toBeTruthy();
    expect(screen.queryByText('syncing now')).toBeNull();
  });

  it('names the device in the revoke confirmation before revoking it', async () => {
    mocks.syncDevices.mockResolvedValue([
      device(),
      device({ deviceId: 'device-b', name: 'Pixel 8', isThisDevice: false, isRoot: false }),
    ]);
    mocks.syncRevokeDevice.mockResolvedValue(2);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Revoke' }));
    // The confirmation is inline and carries the real device name.
    const confirm = await screen.findByRole('button', { name: 'Revoke Pixel 8' });
    expect(mocks.syncRevokeDevice).not.toHaveBeenCalled();

    fireEvent.click(confirm);
    await waitFor(() => {
      expect(mocks.syncRevokeDevice).toHaveBeenCalledWith('device-b');
    });
  });

  it('offers to turn sync back on while it is switched off', async () => {
    mocks.syncStatus.mockResolvedValue(status({ enabled: false }));
    mocks.syncResume.mockResolvedValue(status());
    renderPage();

    expect(await screen.findByText('Sync is off. Everything still works.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Turn sync back on' }));

    await waitFor(() => {
      expect(mocks.syncResume).toHaveBeenCalled();
    });
  });
});
