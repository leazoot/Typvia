// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Snippet, SyncDevice, SyncStatus } from '@typvia/shared';
import { I18nProvider } from '../i18n';
import { SasCompare } from './sas-compare';
import {
  conflictHeadline,
  deviceRowMeta,
  headline,
  isOfflineError,
  revokeConfirmation,
  roundNotice,
  selfDevice,
  selfStatus,
  shapeLabel,
} from './labels';
import { ageLabel, relativeTime } from './relative-time';
import { backlogLabel, conflictCountLabel, syncBadge } from './use-sync-settings';

const NOW = 1_700_000_000_000;

function device(overrides: Partial<SyncDevice> = {}): SyncDevice {
  return {
    deviceId: 'd-1',
    name: 'Pixel 8',
    platform: 'android',
    createdAt: NOW - 86_400_000,
    revokedAt: null,
    verified: true,
    isThisDevice: false,
    isRoot: false,
    ...overrides,
  };
}

function status(overrides: Partial<SyncStatus> = {}): SyncStatus {
  return {
    available: true,
    configured: true,
    enabled: true,
    serverUrl: 'https://sync.example.com',
    accountId: 'acct-1',
    deviceId: 'd-0',
    deviceName: 'This Mac',
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

function snippet(body: string | null): Snippet {
  return {
    id: 's-1',
    title: 'Weekly update',
    body,
    snippetType: 'text',
    securityLevel: body === null ? 'sensitive' : 'normal',
    description: null,
    folderId: null,
    trigger: ';weekly',
    triggerMode: 'delimiter',
    language: null,
    isFavorite: false,
    isPinned: false,
    isEnabled: true,
    createdAt: 1,
    updatedAt: NOW - 120_000,
    lastUsedAt: null,
    usageCount: 0,
    version: 2,
    deletedAt: null,
  };
}

afterEach(cleanup);

describe('shared sync labels', () => {
  it('counts the devices the text lives on, and says so when sync is off', () => {
    expect(headline(0, true, 'en')).toBe('Your text lives on this device.');
    expect(headline(1, true, 'en')).toBe('Your text lives on one device.');
    expect(headline(3, true, 'en')).toBe('Your text lives on 3 devices.');
    expect(headline(3, false, 'en')).toBe('Sync is off. Everything still works.');
  });

  it('reports a round in the user terms, conflicts first', () => {
    expect(roundNotice(0, 0, 0, 'en')).toBe('Already up to date.');
    expect(roundNotice(2, 5, 0, 'en')).toBe('Sent 2, received 5.');
    expect(roundNotice(2, 5, 1, 'en')).toBe(
      '1 snippet changed in two places and needs your decision.',
    );
    expect(roundNotice(0, 0, 2, 'en')).toBe(
      '2 snippets changed in two places and need your decision.',
    );
  });

  it('lets offline win over syncing on this device row', () => {
    expect(selfStatus(true, true, NOW, NOW, 'en')).toBe('offline');
    expect(selfStatus(true, false, NOW, NOW, 'en')).toBe('syncing now');
    expect(selfStatus(false, false, null, NOW, 'en')).toBe('not synced yet');
    expect(selfStatus(false, false, NOW - 120_000, NOW, 'en')).toBe('2 min ago');
  });

  it('names the role, the trust and any pending decision on a device row', () => {
    expect(deviceRowMeta(device(), 0, NOW, 'en')).toBe('paired · android');
    expect(deviceRowMeta(device({ verified: false }), 0, NOW, 'en')).toBe(
      'paired · android · unverified',
    );
    expect(deviceRowMeta(device({ isRoot: true }), 0, NOW, 'en')).toBe('first device · android');
    expect(deviceRowMeta(device({ isThisDevice: true }), 2, NOW, 'en')).toBe(
      '2 conflicts to resolve',
    );
    expect(deviceRowMeta(device({ revokedAt: NOW - 86_400_000 }), 0, NOW, 'en')).toBe(
      'revoked yesterday',
    );
    // The status-only row carries no platform; the separator must not dangle.
    expect(deviceRowMeta(device({ isThisDevice: true, platform: '' }), 0, NOW, 'en')).toBe(
      'this device',
    );
  });

  it('never shows a secret body when describing a conflict version', () => {
    expect(shapeLabel(snippet('one\ntwo three'), false, 'en')).toBe('2 lines · 3 words');
    expect(shapeLabel(snippet('one'), true, 'en')).toBe('encrypted · not shown');
    expect(shapeLabel(snippet(null), false, 'en')).toBe('encrypted · not shown');
  });

  it('names the real device in a destructive confirmation', () => {
    expect(revokeConfirmation('Pixel 8', 'en').startsWith('Revoke Pixel 8?')).toBe(true);
  });

  it('builds this device own row from the status when the server is silent', () => {
    const self = selfDevice(status({ deviceId: 'd-9', deviceName: 'This iPhone' }));
    expect(self.deviceId).toBe('d-9');
    expect(self.name).toBe('This iPhone');
    expect(self.isThisDevice).toBe(true);
  });

  it('reads only the unavailable code as offline', () => {
    expect(isOfflineError(new Error('boom'))).toBe(false);
    expect(isOfflineError(null)).toBe(false);
  });

  it('phrases the conflict count for both surfaces', () => {
    expect(conflictHeadline(1, 'en')).toBe('One snippet changed in two places');
    expect(conflictHeadline(4, 'en')).toBe('4 snippets changed in two places');
    expect(conflictCountLabel(1, 'en')).toBe('1 snippet changed in two places');
    expect(backlogLabel(0, 'en')).toBe('Nothing waiting to send');
    expect(backlogLabel(1, 'en')).toBe('1 change waiting to send');
    expect(backlogLabel(7, 'en')).toBe('7 changes waiting to send');
  });

  it('pairs every badge colour with a word', () => {
    expect(syncBadge(status({ available: false }), 'en')).toEqual({
      kind: 'warning',
      label: 'Unavailable',
    });
    expect(syncBadge(status({ configured: false }), 'en')).toEqual({
      kind: 'warning',
      label: 'Not set up',
    });
    expect(syncBadge(status({ enabled: false }), 'en')).toEqual({ kind: 'warning', label: 'Off' });
    expect(syncBadge(status({ conflictCount: 2 }), 'en')).toEqual({
      kind: 'warning',
      label: '2 to resolve',
    });
    expect(syncBadge(status(), 'en')).toEqual({ kind: 'success', label: 'On' });
  });

  it('turns timestamps into what the drawing is about', () => {
    expect(relativeTime(NOW, NOW, 'en')).toBe('just now');
    expect(relativeTime(NOW - 3_600_000, NOW, 'en')).toBe('1 h ago');
    expect(relativeTime(NOW - 3 * 86_400_000, NOW, 'en')).toBe('3 days ago');
    expect(ageLabel(NOW, NOW, 'en')).toBe('today');
    expect(ageLabel(NOW - 60 * 86_400_000, NOW, 'en')).toBe('2 months ago');
    expect(ageLabel(NOW - 800 * 86_400_000, NOW, 'en')).toBe('2 years ago');
  });

  it('renders the same labels in Chinese under the zh locale', () => {
    expect(headline(3, true, 'zh')).toBe('你的文本存放在 3 台设备上。');
    expect(roundNotice(2, 5, 0, 'zh')).toBe('已发送 2 条，接收 5 条。');
    expect(shapeLabel(snippet('one'), true, 'zh')).toBe('已加密 · 不显示');
    expect(selfStatus(false, true, NOW, NOW, 'zh')).toBe('离线');
    expect(syncBadge(status(), 'zh')).toEqual({ kind: 'success', label: '已开启' });
    expect(backlogLabel(7, 'zh')).toBe('7 项变更等待发送');
    expect(relativeTime(NOW - 120_000, NOW, 'zh')).toBe('2 分钟前');
    expect(ageLabel(NOW - 60 * 86_400_000, NOW, 'zh')).toBe('2 个月前');
  });
});

describe('SasCompare', () => {
  it('exposes the code for reading aloud and states the anti-relay rule', () => {
    render(
      <SasCompare sas="ABCD EFGH JKLM NPQR" heading="Admit Pixel 8?" detail="Pixel 8 · android" />,
    );

    expect(screen.getByLabelText('Verification code ABCD EFGH JKLM NPQR')).toBeDefined();
    expect(screen.getByText('Admit Pixel 8?')).toBeDefined();
    expect(screen.getByText(/must be identical on both devices/)).toBeDefined();
    // One language at a time: no Chinese twin next to the English rule.
    expect(screen.queryByText(/两台设备/)).toBeNull();
  });

  it('renders the single Chinese rule under the zh locale', () => {
    render(
      <I18nProvider locale="zh">
        <SasCompare sas="ABCD EFGH JKLM NPQR" heading="Admit Pixel 8?" detail="Pixel 8 · android" />
      </I18nProvider>,
    );

    expect(screen.getByLabelText('验证码 ABCD EFGH JKLM NPQR')).toBeDefined();
    expect(screen.getByText('核对短码')).toBeDefined();
    expect(screen.getByText(/两台设备上的四组字符必须完全一致/)).toBeDefined();
    expect(screen.queryByText(/must be identical on both devices/)).toBeNull();
  });
});
