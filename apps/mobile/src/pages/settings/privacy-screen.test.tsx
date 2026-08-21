// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MobileEgressLogScreen, MobilePrivacyScreen } from './privacy-screen';

const ipc = vi.hoisted(() => ({
  aiEgressLogList: vi.fn(),
}));

vi.mock('@typvia/shared', () => ipc);

function entry(id: number, overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id,
    occurredAt: Date.now(),
    providerId: 'p1',
    requestClass: 'completion',
    requestBytes: 512,
    ...overrides,
  };
}

beforeEach(() => {
  ipc.aiEgressLogList.mockResolvedValue({ entries: [], total: 0 });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('MobilePrivacyScreen', () => {
  it('states the claims in the negative with the receipt beside them', async () => {
    ipc.aiEgressLogList.mockResolvedValue({ entries: [entry(1), entry(2)], total: 7 });
    render(<MobilePrivacyScreen onBack={() => {}} onEgressLog={() => {}} />);

    expect(await screen.findByText('No analytics, no crash reporting')).toBeDefined();
    expect(screen.getByText('The keyboard records nothing')).toBeDefined();
    expect(screen.getByText('Vault items never reach AI')).toBeDefined();
    expect(screen.getByText('2 today · 7 all time')).toBeDefined();
    expect(screen.getByText('Nothing. Ever.')).toBeDefined();
    expect(screen.getByText(/can’t be turned off/)).toBeDefined();
  });

  it('opens the egress log from the AI requests row', async () => {
    const onEgressLog = vi.fn();
    render(<MobilePrivacyScreen onBack={() => {}} onEgressLog={onEgressLog} />);

    fireEvent.click(await screen.findByRole('button', { name: /AI requests/ }));
    expect(onEgressLog).toHaveBeenCalledTimes(1);
  });

  it('an unreadable log reads as unreadable, never as zero', async () => {
    ipc.aiEgressLogList.mockRejectedValue(new Error('io'));
    render(<MobilePrivacyScreen onBack={() => {}} onEgressLog={() => {}} />);

    expect(await screen.findByText('unreadable right now')).toBeDefined();
  });
});

describe('MobileEgressLogScreen', () => {
  it('lists metadata only and pages explicitly', async () => {
    ipc.aiEgressLogList.mockResolvedValue({
      entries: [
        entry(2, { requestBytes: 64 }),
        entry(1, { requestClass: 'connectivity', requestBytes: 0 }),
      ],
      total: 60,
    });
    render(<MobileEgressLogScreen onBack={() => {}} />);

    expect(await screen.findByText('AI request')).toBeDefined();
    expect(screen.getByText('Connectivity check')).toBeDefined();
    expect(screen.getAllByText('p1').length).toBe(2);
    expect(screen.getByText(/64 B/)).toBeDefined();
    expect(ipc.aiEgressLogList).toHaveBeenCalledWith(50, 0);

    fireEvent.click(screen.getByRole('button', { name: 'Show older entries' }));
    await waitFor(() => {
      expect(ipc.aiEgressLogList).toHaveBeenCalledWith(50, 2);
    });
  });

  it('an empty log says no request ever left', async () => {
    render(<MobileEgressLogScreen onBack={() => {}} />);

    expect(await screen.findByText(/no AI request has ever left this phone/)).toBeDefined();
  });
});
