// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/** Stable error codes shared with the Rust IPC layer (error.rs). */
export type IpcErrorCode =
  | 'validation'
  | 'conflict'
  | 'not_found'
  | 'permission_denied'
  | 'rule_blocked'
  | 'unavailable'
  | 'system';

/**
 * Typed IPC failure. `validation` is user-correctable input, `conflict` /
 * `not_found` / `permission_denied` / `rule_blocked` are business outcomes
 * (`permission_denied` means injection needs OS permission — callers fall
 * back to copy; `rule_blocked` means an app rule refused the action and the
 * panel shows the rule notice instead; `unavailable` means a remote service
 * could not be reached, which the UI shows as the offline state rather than a
 * failure), `system` is an internal failure with a deliberately generic
 * message.
 */
export class IpcError extends Error {
  readonly code: IpcErrorCode;

  constructor(code: IpcErrorCode, message: string) {
    super(message);
    this.name = 'IpcError';
    this.code = code;
  }
}

const CODES: readonly IpcErrorCode[] = [
  'validation',
  'conflict',
  'not_found',
  'permission_denied',
  'rule_blocked',
  'unavailable',
  'system',
];

/** Normalizes whatever the bridge rejected with into an IpcError. */
export function toIpcError(raw: unknown): IpcError {
  if (typeof raw === 'object' && raw !== null && 'code' in raw && 'message' in raw) {
    const { code, message } = raw as { code: unknown; message: unknown };
    if (typeof message === 'string' && CODES.includes(code as IpcErrorCode)) {
      return new IpcError(code as IpcErrorCode, message);
    }
  }
  return new IpcError('system', 'unexpected IPC failure');
}
