// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import type { SyncStatus } from '@typvia/shared';
import { TRIGGER_MAX_CHARS, TRIGGER_MIN_CHARS, syncLine, triggerChars } from './library-model';

function syncStatus(over: Partial<SyncStatus> = {}): SyncStatus {
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
    lastSyncAt: null,
    vaultReady: true,
    vaultUnlocked: false,
    recoveryExportedAt: null,
    recoveryCatchupPending: false,
    transportKind: 'server',
    ...over,
  };
}

describe('triggerChars', () => {
  it('fits the longest trigger whole', () => {
    expect(triggerChars([';api', ';mail1', null])).toBe(6);
  });

  it('keeps short and missing triggers at the narrowest column', () => {
    expect(triggerChars(['/fu', null])).toBe(TRIGGER_MIN_CHARS);
    expect(triggerChars([])).toBe(TRIGGER_MIN_CHARS);
  });

  it('stops widening at the cap, leaving longer triggers to ellipsize', () => {
    expect(triggerChars([';a-very-long-trigger-name'])).toBe(TRIGGER_MAX_CHARS);
  });

  it('counts characters, not UTF-16 units', () => {
    expect(triggerChars(['😀😀😀😀😀'])).toBe(5);
  });
});

describe('syncLine', () => {
  it('reads as paused when sync is switched off', () => {
    expect(syncLine(syncStatus({ enabled: false }), 0, 'en')?.tone).toBe('paused');
  });

  it('warns while changes are still waiting to be sent', () => {
    expect(syncLine(syncStatus({ pendingBacklog: 3 }), 0, 'en')).toEqual({
      text: '3 not sent yet',
      tone: 'warn',
    });
  });

  it('stays idle, not asleep, when sync was never set up', () => {
    expect(syncLine(syncStatus({ configured: false }), 0, 'en')?.tone).toBe('idle');
  });
});
