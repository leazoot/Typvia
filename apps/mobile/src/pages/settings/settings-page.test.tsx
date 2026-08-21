// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { UiPrefsProvider } from '@typvia/ui';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsPage } from './settings-page';

const ipc = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  aiProviderList: vi.fn(),
}));

vi.mock('@typvia/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof SharedModule>();
  return { ...actual, ...ipc };
});

function status(): SharedModule.SyncStatus {
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
  };
}

beforeEach(() => {
  localStorage.removeItem('tv.ui.locale');
  localStorage.removeItem('tv.ui.theme');
  ipc.syncStatus.mockResolvedValue(status());
  ipc.aiProviderList.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.removeItem('tv.ui.locale');
  localStorage.removeItem('tv.ui.theme');
});

function renderPage() {
  return render(
    <UiPrefsProvider>
      <SettingsPage />
    </UiPrefsProvider>,
  );
}

describe('mobile Settings — Appearance', () => {
  it('switches the whole UI language from the Language row', async () => {
    renderPage();

    const group = await screen.findByRole('group', { name: 'Language' });
    // Default is System; the explicit choices sit beside it.
    expect(within(group).getByRole('button', { name: 'System' }).getAttribute('aria-pressed')).toBe(
      'true',
    );

    fireEvent.click(within(group).getByRole('button', { name: '中文' }));

    expect(localStorage.getItem('tv.ui.locale')).toBe('zh');
    // The page itself reads in Chinese now — one language everywhere.
    expect(await screen.findByRole('heading', { name: '设置' })).toBeDefined();
    expect(screen.getByRole('group', { name: '语言' })).toBeDefined();
  });

  it('stores the theme preference from the Theme row', async () => {
    renderPage();

    const group = await screen.findByRole('group', { name: 'Theme' });
    const dark = within(group).getByRole('button', { name: 'Dark' });
    expect(dark.getAttribute('aria-pressed')).toBe('false');

    fireEvent.click(dark);

    expect(localStorage.getItem('tv.ui.theme')).toBe('dark');
    await waitFor(() => {
      expect(dark.getAttribute('aria-pressed')).toBe('true');
    });
  });
});
