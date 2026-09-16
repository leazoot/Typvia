// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import type { SyncStatus } from '@typvia/shared';
import type * as SharedModule from '@typvia/shared';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PairPage } from './pair-page';

const mocks = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  pairingSas: vi.fn(),
  pairingApprove: vi.fn(),
  pairingBegin: vi.fn(),
  pairingBeginWebdav: vi.fn(),
  pairingPoll: vi.fn(),
  pairingFinalize: vi.fn(),
  webdavCredentials: vi.fn(),
  pairingCancel: vi.fn(),
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

function renderPage() {
  return render(
    <MemoryRouter initialEntries={['/sync/pair']}>
      <PairPage />
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('Pairing — the admitting device', () => {
  beforeEach(() => {
    mocks.syncStatus.mockResolvedValue(status());
    mocks.pairingSas.mockResolvedValue({
      sas: 'ABCD EFGH JKLM NPQR',
      deviceName: 'Pixel 8',
      platform: 'android',
      vaultReady: true,
    });
    mocks.pairingApprove.mockResolvedValue(undefined);
  });

  it('cannot admit a device before the short code has been compared', async () => {
    renderPage();
    const field = await screen.findByLabelText('Pairing code from the new device');

    fireEvent.change(field, { target: { value: 'TYPVIA-PAIR.V1.abc' } });

    // Reading the code is the only action available; nothing admits yet.
    expect(screen.queryByRole('button', { name: /admit it/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    expect(await screen.findByText('Admit Pixel 8?')).toBeTruthy();
    expect(screen.getByLabelText('Verification code ABCD EFGH JKLM NPQR')).toBeTruthy();
    expect(mocks.pairingApprove).not.toHaveBeenCalled();
  });

  it('states that the four groups must match on both devices', async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    expect(await screen.findByText(/must be identical on both devices/)).toBeTruthy();
  });

  it('admits the device only after the user confirms the match', async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    fireEvent.click(await screen.findByRole('button', { name: /The characters match — admit it/ }));

    await waitFor(() => {
      expect(mocks.pairingApprove).toHaveBeenCalledWith('TYPVIA-PAIR.V1.abc', true);
    });
  });

  it('states honestly what withholding vault access means', async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Vault access: allowed' }));

    expect(await screen.findByText(/show your secrets as locked rows/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /The characters match — admit it/ }));
    await waitFor(() => {
      expect(mocks.pairingApprove).toHaveBeenCalledWith('TYPVIA-PAIR.V1.abc', false);
    });
  });

  it('goes back to the code when the user says the codes do not match', async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    fireEvent.click(await screen.findByRole('button', { name: 'They do not match' }));

    expect(await screen.findByLabelText('Pairing code from the new device')).toBeTruthy();
    expect(mocks.pairingApprove).not.toHaveBeenCalled();
  });
});

describe('Pairing — the joining device', () => {
  beforeEach(() => {
    // The core composes the credential string; the page only carries it.
    mocks.webdavCredentials.mockResolvedValue('basic:FAKE_user:FAKE_dav_pw');
    mocks.syncStatus.mockResolvedValue(status({ configured: false, enabled: false }));
    mocks.pairingBegin.mockResolvedValue({ code: 'TYPVIA-PAIR.V1.abc', sessionId: 'session-1' });
    mocks.pairingPoll.mockResolvedValue(null);
    mocks.pairingFinalize.mockResolvedValue(status());
  });

  it('joins over WebDAV with credentials and no manual account id', async () => {
    mocks.pairingBeginWebdav.mockResolvedValue({ code: 'TYPVIA-CODE', sessionId: 's-1' });
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
    fireEvent.click(screen.getByRole('button', { name: 'Show my pairing code' }));

    await waitFor(() =>
      // What is asserted is that the composed string is handed on, not what
      // it looks like: the shape is the transport's, not this page's.
      expect(mocks.pairingBeginWebdav).toHaveBeenCalledWith(
        'https://dav.example.com/typvia',
        'basic:FAKE_user:FAKE_dav_pw',
      ),
    );
  });

  it('shows the code as a scannable image and as text, and waits', async () => {
    const { container } = renderPage();
    fireEvent.change(await screen.findByLabelText('Server address'), {
      target: { value: 'https://sync.example.com' },
    });
    fireEvent.change(screen.getByLabelText('Account id'), { target: { value: 'account-1' } });

    fireEvent.click(screen.getByRole('button', { name: 'Show my pairing code' }));

    expect(await screen.findByRole('img', { name: 'Pairing code' })).toBeTruthy();
    expect((screen.getByLabelText('Pairing code text') as HTMLTextAreaElement).value).toBe(
      'TYPVIA-PAIR.V1.abc',
    );
    expect(screen.getByText(/Nothing has been installed yet/)).toBeTruthy();
    expect(container.querySelector('svg')).not.toBeNull();
  });

  it('installs nothing until the user confirms the short code matches', async () => {
    mocks.pairingPoll.mockResolvedValue({
      sas: 'ABCD EFGH JKLM NPQR',
      rootFingerprint: 'AB12-CD34',
    });
    renderPage();
    fireEvent.change(await screen.findByLabelText('Server address'), {
      target: { value: 'https://sync.example.com' },
    });
    fireEvent.change(screen.getByLabelText('Account id'), { target: { value: 'account-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Show my pairing code' }));

    // The offer arrives on the poll; the page stops at the comparison.
    const confirm = await screen.findByRole(
      'button',
      { name: /The characters match — join/ },
      { timeout: 4000 },
    );
    expect(mocks.pairingFinalize).not.toHaveBeenCalled();

    fireEvent.click(confirm);
    await waitFor(() => {
      expect(mocks.pairingFinalize).toHaveBeenCalledWith(null);
    });
  });
});

describe('Pairing — locale', () => {
  it('renders in Chinese when wrapped in the zh locale', async () => {
    mocks.syncStatus.mockResolvedValue(status());
    render(
      <I18nProvider locale="zh">
        <MemoryRouter initialEntries={['/sync/pair']}>
          <PairPage />
        </MemoryRouter>
      </I18nProvider>,
    );

    expect(await screen.findByText('配对设备。')).toBeTruthy();
    expect(screen.getByLabelText('来自新设备的配对码')).toBeTruthy();
    expect(screen.getByRole('button', { name: '返回设备列表' })).toBeTruthy();
  });
});
