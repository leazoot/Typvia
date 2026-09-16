// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type * as SharedModule from '@typvia/shared';
import type { InsertionPause, Snippet, SyncStatus, VaultStatus } from '@typvia/shared';
import { TrayApp } from './tray-app';

let showHandler: (() => void) | null = null;

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: () => void) => {
    if (event === 'tray:show') showHandler = handler;
    return Promise.resolve(() => undefined);
  },
}));

function snippet(id: string, body: string): Snippet {
  return {
    id,
    title: `Title ${id}`,
    body,
    snippetType: 'text',
    securityLevel: 'normal',
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

function sync(over: Partial<SyncStatus> = {}): SyncStatus {
  return {
    available: true,
    configured: true,
    enabled: true,
    serverUrl: 'https://sync.example.test',
    accountId: 'account-1',
    deviceId: 'device-1',
    deviceName: 'Test Mac',
    keyGeneration: 1,
    pendingBacklog: 0,
    conflictCount: 0,
    lastSyncAt: 1,
    vaultReady: true,
    vaultUnlocked: false,
    recoveryExportedAt: null,
    recoveryCatchupPending: false,
    transportKind: 'server',
    ...over,
  };
}

function vault(unlocked: boolean): VaultStatus {
  return {
    initialized: true,
    unlocked,
    unlockedAt: unlocked ? 1 : null,
    lastActivityAt: unlocked ? 1 : null,
    idleTimeoutMs: 300_000,
  };
}

const trayInsert = vi.fn((id: string, method?: string) => {
  void id;
  void method;
  return Promise.resolve();
});
const hideTray = vi.fn(() => Promise.resolve());
const pauseInsertion = vi.fn((minutes: number) =>
  Promise.resolve<InsertionPause>({ remainingMs: minutes * 60_000 }),
);
const resumeInsertion = vi.fn(() => Promise.resolve<InsertionPause>({ remainingMs: null }));
const vaultLock = vi.fn(() => Promise.resolve(vault(false)));
const vaultStatus = vi.fn(() => Promise.resolve(vault(false)));
const syncStatus = vi.fn(() => Promise.resolve(sync()));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return {
    ...actual,
    syncStatus: () => syncStatus(),
    libraryCounts: () =>
      Promise.resolve({ total: 12, recent: 0, starred: 0, unsorted: 0, trash: 0, folders: [] }),
    trayResults: () => Promise.resolve([snippet('a', 'First line\nsecond'), snippet('b', '')]),
    vaultStatus: () => vaultStatus(),
    insertionPauseStatus: () => Promise.resolve({ remainingMs: null }),
    autostartStatus: () => Promise.resolve(false),
    trayInsert: (id: string, method?: string) => trayInsert(id, method),
    trayPresent: () => Promise.resolve(),
    hideTray: () => hideTray(),
    pauseInsertion: (minutes: number) => pauseInsertion(minutes),
    resumeInsertion: () => resumeInsertion(),
    vaultLock: () => vaultLock(),
  };
});

async function open() {
  render(<TrayApp />);
  await act(async () => {
    showHandler?.();
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  showHandler = null;
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('TrayApp', () => {
  it('says how sync stands and lists the recent snippets with their chords', async () => {
    await open();
    expect(screen.getByText('Synced')).toBeDefined();
    expect(screen.getByText('12 snippets')).toBeDefined();
    expect(screen.getByText('First line')).toBeDefined();
    // A snippet with no body goes by its title.
    expect(screen.getByText('Title b')).toBeDefined();
    expect(screen.getByText('Ctrl Alt 2')).toBeDefined();
  });

  it('reads not synced while changes wait to be sent', async () => {
    syncStatus.mockResolvedValueOnce(sync({ pendingBacklog: 2 }));
    await open();
    expect(screen.getByText('Not synced')).toBeDefined();
  });

  it('inserts a row on click and on its chord', async () => {
    await open();
    fireEvent.click(screen.getByText('First line'));
    expect(trayInsert).toHaveBeenCalledWith('a', 'paste');
    fireEvent.keyDown(window, { code: 'Digit2', ctrlKey: true, altKey: true });
    expect(trayInsert).toHaveBeenLastCalledWith('b', 'paste');
  });

  it('puts itself away on esc', async () => {
    await open();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(hideTray).toHaveBeenCalledTimes(1);
  });

  it('pauses inserting for an hour and offers to resume', async () => {
    await open();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Pause inserting for 1 hour' }));
      await Promise.resolve();
    });
    expect(pauseInsertion).toHaveBeenCalledWith(60);
    expect(screen.getByRole('button', { name: 'Resume inserting · 60 min left' })).toBeDefined();
  });

  it('offers to lock the vault only while it is unlocked', async () => {
    await open();
    expect(screen.queryByRole('button', { name: 'Lock the vault' })).toBeNull();
    cleanup();
    vaultStatus.mockResolvedValueOnce(vault(true));
    await open();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Lock the vault' }));
      await Promise.resolve();
    });
    expect(vaultLock).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('button', { name: 'Lock the vault' })).toBeNull();
  });
});
