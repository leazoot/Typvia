// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import { IpcError } from './error';
import { ipcErrorCopy } from './error-copy';

describe('ipcErrorCopy', () => {
  it('translates known backend sentences exactly', () => {
    expect(ipcErrorCopy(new IpcError('conflict', 'trigger already in use'))).toEqual([
      'trigger already in use',
      '触发词已被占用',
    ]);
    expect(ipcErrorCopy(new IpcError('permission_denied', 'unlock the vault first'))).toEqual([
      'unlock the vault first',
      '请先解锁保险库',
    ]);
    expect(ipcErrorCopy(new IpcError('unavailable', 'the server could not be reached'))).toEqual([
      'the server could not be reached',
      '无法连接服务器',
    ]);
  });

  it('keeps the English and falls back per code for unknown sentences', () => {
    const [en, zh] = ipcErrorCopy(new IpcError('validation', 'fill in the required field: name'));
    expect(en).toBe('fill in the required field: name');
    expect(zh).toBe('输入内容无法使用,请检查后重试');
  });

  it('collapses non-IPC failures to the generic system line', () => {
    const [en, zh] = ipcErrorCopy(new Error('boom'));
    expect(en).toBe('boom');
    expect(zh).toBe('内部错误——你的数据未受影响');
    expect(ipcErrorCopy(undefined)[1]).toBe('内部错误——你的数据未受影响');
  });

  it('never leaves a zh reading empty for any code', () => {
    const codes = [
      'validation',
      'conflict',
      'not_found',
      'permission_denied',
      'rule_blocked',
      'unavailable',
      'system',
    ] as const;
    for (const code of codes) {
      const [, zh] = ipcErrorCopy(new IpcError(code, 'some sentence this table does not know'));
      expect(zh.length).toBeGreaterThan(0);
      expect(/[一-鿿]/.test(zh)).toBe(true);
    }
  });
});
