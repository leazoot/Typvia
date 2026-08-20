// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type * as SharedModule from '@typvia/shared';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MobilePairPage } from './pair-page';

const ipc = vi.hoisted(() => ({
  pairingSas: vi.fn(),
  pairingApprove: vi.fn(),
  pairingBegin: vi.fn(),
  pairingPoll: vi.fn(),
  pairingFinalize: vi.fn(),
  pairingCancel: vi.fn(),
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

const noop = () => undefined;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('mobile pairing — admitting side', () => {
  it('never admits a device before the characters were confirmed', async () => {
    ipc.pairingSas.mockResolvedValue({
      sas: 'ABCD EFGH JKLM NPQR',
      deviceName: 'This Mac',
      platform: 'macos',
      vaultReady: false,
    });

    render(<MobilePairPage status={status()} onBack={noop} onDone={noop} />);

    fireEvent.change(screen.getByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));

    // Reading a code admits nothing; the comparison screen is the gate.
    expect(await screen.findByLabelText('Verification code ABCD EFGH JKLM NPQR')).toBeDefined();
    expect(ipc.pairingApprove).not.toHaveBeenCalled();
    expect(screen.getByText('Admit This Mac?')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'The characters match — admit it' }));
    await waitFor(() => {
      expect(ipc.pairingApprove).toHaveBeenCalledWith('TYPVIA-PAIR.V1.abc', false);
    });
  });

  it('returns to the code entry when the characters do not match', async () => {
    ipc.pairingSas.mockResolvedValue({
      sas: 'ABCD EFGH JKLM NPQR',
      deviceName: 'This Mac',
      platform: 'macos',
      vaultReady: false,
    });

    render(<MobilePairPage status={status()} onBack={noop} onDone={noop} />);
    fireEvent.change(screen.getByLabelText('Pairing code from the new device'), {
      target: { value: 'TYPVIA-PAIR.V1.abc' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Read the code' }));
    fireEvent.click(await screen.findByRole('button', { name: 'They do not match' }));

    expect(await screen.findByLabelText('Pairing code from the new device')).toBeDefined();
    expect(ipc.pairingApprove).not.toHaveBeenCalled();
  });
});

describe('mobile pairing — joining side', () => {
  it('shows the code as a QR and as text, and installs nothing while waiting', async () => {
    ipc.pairingBegin.mockResolvedValue({ code: 'TYPVIA-PAIR.V1.xyz', sessionId: 'sess-1' });
    ipc.pairingPoll.mockResolvedValue(null);

    render(<MobilePairPage status={status({ configured: false })} onBack={noop} onDone={noop} />);

    fireEvent.change(screen.getByLabelText('Server address'), {
      target: { value: 'https://sync.example.com' },
    });
    fireEvent.change(screen.getByLabelText('Account id'), { target: { value: 'acct-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Show my pairing code' }));

    expect(await screen.findByRole('img', { name: 'Pairing code' })).toBeDefined();
    expect((screen.getByLabelText('Pairing code text') as HTMLTextAreaElement).value).toBe(
      'TYPVIA-PAIR.V1.xyz',
    );
    expect(screen.getByText(/Nothing has been installed yet/)).toBeDefined();
    expect(ipc.pairingFinalize).not.toHaveBeenCalled();
  });

  it('installs nothing until the offer characters were confirmed', async () => {
    ipc.pairingBegin.mockResolvedValue({ code: 'TYPVIA-PAIR.V1.xyz', sessionId: 'sess-1' });
    ipc.pairingPoll.mockResolvedValue({ sas: 'WXYZ 2345 6789 ABCD', rootFingerprint: 'AA-BB' });
    ipc.pairingFinalize.mockResolvedValue(status());

    render(<MobilePairPage status={status({ configured: false })} onBack={noop} onDone={noop} />);
    fireEvent.change(screen.getByLabelText('Server address'), {
      target: { value: 'https://sync.example.com' },
    });
    fireEvent.change(screen.getByLabelText('Account id'), { target: { value: 'acct-1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Show my pairing code' }));

    const confirm = await screen.findByRole(
      'button',
      { name: 'The characters match — join' },
      { timeout: 4000 },
    );
    expect(screen.getByLabelText('Verification code WXYZ 2345 6789 ABCD')).toBeDefined();
    expect(ipc.pairingFinalize).not.toHaveBeenCalled();

    fireEvent.click(confirm);
    await waitFor(() => {
      expect(ipc.pairingFinalize).toHaveBeenCalledWith(null);
    });
  });
});
