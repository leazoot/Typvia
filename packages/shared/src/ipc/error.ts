/** Stable error codes shared with the Rust IPC layer (error.rs). */
export type IpcErrorCode = 'validation' | 'conflict' | 'not_found' | 'permission_denied' | 'system';

/**
 * Typed IPC failure. `validation` is user-correctable input, `conflict` /
 * `not_found` / `permission_denied` are business outcomes (the last one means
 * injection needs OS permission — callers fall back to copy), `system` is an
 * internal failure with a deliberately generic message.
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
