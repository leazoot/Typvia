// @vitest-environment jsdom
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { createSnippet, getSnippet, searchSnippets } from './client';
import { IpcError, toIpcError } from './error';
import type { Snippet } from './types';

afterEach(clearMocks);

const SNIPPET: Snippet = {
  id: 's-1',
  title: 'Docker tail logs',
  body: 'docker logs -f',
  snippetType: 'command',
  securityLevel: 'normal',
  description: null,
  folderId: null,
  trigger: ';dockerlog',
  triggerMode: 'delimiter',
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

describe('typed IPC client', () => {
  it('sends the command name and payload the Rust layer expects', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      if (cmd === 'snippet_create') return SNIPPET;
      if (cmd === 'search_snippets') return [];
      throw { code: 'not_found', message: 'not found' };
    });

    const created = await createSnippet({
      title: 'Docker tail logs',
      body: 'docker logs -f',
      snippetType: 'command',
      trigger: ';dockerlog',
      triggerMode: 'delimiter',
    });
    expect(created.id).toBe('s-1');
    expect(seen[0]?.cmd).toBe('snippet_create');
    expect(seen[0]?.args).toEqual({
      input: {
        title: 'Docker tail logs',
        body: 'docker logs -f',
        snippetType: 'command',
        trigger: ';dockerlog',
        triggerMode: 'delimiter',
      },
    });

    await searchSnippets('docker', 10, 0);
    expect(seen[1]?.cmd).toBe('search_snippets');
    expect(seen[1]?.args).toEqual({ query: 'docker', limit: 10, offset: 0 });
  });

  it('normalizes structured rejections into typed IpcError', async () => {
    mockIPC(() => {
      throw { code: 'conflict', message: 'trigger already in use' };
    });
    const error = await getSnippet('s-1').catch((e: unknown) => e);
    expect(error).toBeInstanceOf(IpcError);
    expect((error as IpcError).code).toBe('conflict');
    expect((error as IpcError).message).toBe('trigger already in use');
  });

  it('maps unknown rejection shapes to a generic system error', () => {
    const error = toIpcError('boom');
    expect(error.code).toBe('system');
    expect(error.message).toBe('unexpected IPC failure');
  });
});
