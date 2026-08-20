// @vitest-environment jsdom
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import {
  appRuleCreate,
  appRuleDelete,
  appRuleList,
  appRuleUpdate,
  backupExport,
  backupExportFile,
  backupRestore,
  clipboardReadText,
  copySnippet,
  createSnippet,
  espansoDisable,
  espansoImport,
  espansoStatus,
  espansoSync,
  getSnippet,
  hidePanel,
  historyGet,
  historyList,
  historyRestore,
  injectSnippet,
  libraryCounts,
  listSnippetPage,
  mobileBootstrap,
  onboardingComplete,
  onboardingStatus,
  openKeyboardSettings,
  panelCopySecret,
  panelInsert,
  panelInsertSecret,
  panelInsertTemplate,
  panelReady,
  panelResults,
  searchSnippets,
  shareInboxIngest,
  browserIntegrationDisable,
  searchLibraryDeep,
  semanticModelDelete,
  semanticModelDownload,
  semanticStatus,
  semanticSync,
  browserIntegrationEnable,
  browserIntegrationStatus,
  browserIntegrationSync,
  snapshotRefresh,
  snippetsImport,
  templateFields,
  templatePreview,
  templateRender,
  templateSaveFields,
  templateVariables,
  vaultCopySecret,
  vaultCreateSecret,
  vaultList,
  vaultReveal,
  vaultStatus,
  vaultUnlockPassword,
} from './client';
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

  it('sends vault commands with the wire contract', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    const STATUS = {
      initialized: true,
      unlocked: true,
      unlockedAt: 1000,
      lastActivityAt: 1000,
      idleTimeoutMs: 300000,
    };
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      if (cmd === 'vault_status' || cmd === 'vault_unlock_password') return STATUS;
      if (cmd === 'vault_list') return [];
      if (cmd === 'vault_reveal') return 'sk_test_secret';
      if (cmd === 'vault_copy_secret') return 30_000;
      if (cmd === 'vault_create_secret')
        return { ...SNIPPET, body: null, securityLevel: 'sensitive' };
      throw { code: 'not_found', message: 'not found' };
    });

    await vaultStatus();
    expect(seen[0]?.cmd).toBe('vault_status');

    const unlocked = await vaultUnlockPassword('correct horse');
    expect(unlocked.unlocked).toBe(true);
    expect(seen[1]).toEqual({ cmd: 'vault_unlock_password', args: { password: 'correct horse' } });

    await vaultList(50, 0);
    expect(seen[2]).toEqual({ cmd: 'vault_list', args: { limit: 50, offset: 0 } });

    const revealed = await vaultReveal('s-1');
    expect(revealed).toBe('sk_test_secret');
    expect(seen[3]).toEqual({ cmd: 'vault_reveal', args: { id: 's-1' } });

    // The copy resolves with the clear delay only — never the plaintext.
    const clearMs = await vaultCopySecret('s-1');
    expect(clearMs).toBe(30_000);
    expect(seen[4]).toEqual({ cmd: 'vault_copy_secret', args: { id: 's-1' } });

    const created = await vaultCreateSecret({
      title: 'Prod key',
      body: 'sk_live_x',
      snippetType: 'sensitive',
    });
    expect(created.body).toBeNull();
    expect(seen[5]?.cmd).toBe('vault_create_secret');
    expect(seen[5]?.args).toEqual({
      input: { title: 'Prod key', body: 'sk_live_x', snippetType: 'sensitive' },
    });
  });

  it('sends library page and counts requests with the wire contract', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      if (cmd === 'snippet_list_page') return [SNIPPET];
      return { total: 1, recent: 0, starred: 0, unsorted: 1, trash: 0, folders: [] };
    });

    const page = await listSnippetPage('folder', 'f-1', 'command', 200, 400);
    expect(page).toHaveLength(1);
    expect(seen[0]?.cmd).toBe('snippet_list_page');
    expect(seen[0]?.args).toEqual({
      view: 'folder',
      folderId: 'f-1',
      snippetType: 'command',
      limit: 200,
      offset: 400,
    });

    const counts = await libraryCounts();
    expect(counts.total).toBe(1);
    expect(seen[1]?.cmd).toBe('library_counts');
  });

  it('sends inject and copy requests with the wire contract', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return null;
    });

    await injectSnippet('s-1', 'keystrokes');
    expect(seen[0]?.cmd).toBe('snippet_inject');
    expect(seen[0]?.args).toEqual({ id: 's-1', method: 'keystrokes' });

    // Default method sends null so the host applies its default (paste).
    await injectSnippet('s-2');
    expect(seen[1]?.args).toEqual({ id: 's-2', method: null });

    await copySnippet('s-3');
    expect(seen[2]?.cmd).toBe('snippet_copy');
    expect(seen[2]?.args).toEqual({ id: 's-3' });
  });

  it('sends panel window commands with no payload', async () => {
    const seen: string[] = [];
    mockIPC((cmd) => {
      seen.push(cmd);
      return null;
    });

    await hidePanel();
    await panelReady();
    expect(seen).toEqual(['panel_hide', 'panel_ready']);
  });

  it('sends the panel insert request with the wire contract', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return null;
    });

    await panelInsert('s-1');
    expect(seen[0]?.cmd).toBe('panel_insert');
    expect(seen[0]?.args).toEqual({ id: 's-1', method: null });
  });

  it('sends the sensitive panel deliver requests with the wire contract', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return cmd === 'panel_copy_secret' ? 30_000 : null;
    });

    await panelInsertSecret('sec-1');
    await panelCopySecret('sec-1');
    expect(seen[0]).toEqual({ cmd: 'panel_insert_secret', args: { id: 'sec-1', method: null } });
    expect(seen[1]).toEqual({ cmd: 'panel_copy_secret', args: { id: 'sec-1' } });
  });

  it('sends espanso status/sync/disable with the wire contract', async () => {
    const seen: string[] = [];
    mockIPC((cmd) => {
      seen.push(cmd);
      if (cmd === 'espanso_status') {
        return {
          state: 'running',
          version: '2.4.0',
          configPath: '/x/match/typvia.yml',
          enabled: true,
          triggerCount: 3,
        };
      }
      return { enabled: cmd === 'espanso_sync', triggerCount: cmd === 'espanso_sync' ? 3 : 0 };
    });

    const status = await espansoStatus();
    expect(status.state).toBe('running');
    expect(status.triggerCount).toBe(3);
    expect((await espansoSync()).enabled).toBe(true);
    expect((await espansoDisable()).enabled).toBe(false);
    expect(seen).toEqual(['espanso_status', 'espanso_sync', 'espanso_disable']);
  });

  it('sends the espanso import request with the pasted content', async () => {
    let captured: Record<string, unknown> | undefined;
    mockIPC((cmd, args) => {
      if (cmd === 'espanso_import') captured = args as Record<string, unknown>;
      return { imported: 1, conflicts: [], skipped: [] };
    });
    const result = await espansoImport('matches: []');
    expect(captured).toEqual({ content: 'matches: []' });
    expect(result.imported).toBe(1);
  });

  it('sends backup export and restore with the wire contract', async () => {
    const captured: Record<string, Record<string, unknown>> = {};
    mockIPC((cmd, args) => {
      captured[cmd] = args as Record<string, unknown>;
      if (cmd === 'backup_export') return '{"format":"typvia-backup"}';
      return { snippets: 2, folders: 0, tags: 1, versions: 3, vaultRestored: true };
    });

    const text = await backupExport('pass');
    const report = await backupRestore('pass', text);
    expect(captured['backup_export']).toEqual({ passphrase: 'pass' });
    expect(captured['backup_restore']).toEqual({ passphrase: 'pass', content: text });
    expect(report.snippets).toBe(2);
    expect(report.vaultRestored).toBe(true);
  });

  it('sends the backup file export and reads back the file name', async () => {
    let captured: Record<string, unknown> | undefined;
    mockIPC((cmd, args) => {
      if (cmd === 'backup_export_file') captured = args as Record<string, unknown>;
      return 'Typvia-backup-1754.json';
    });
    const name = await backupExportFile('pass');
    expect(captured).toEqual({ passphrase: 'pass' });
    expect(name).toBe('Typvia-backup-1754.json');
  });

  it('sends history list/get/restore with the wire contract', async () => {
    const captured: Record<string, Record<string, unknown>> = {};
    mockIPC((cmd, args) => {
      captured[cmd] = args as Record<string, unknown>;
      if (cmd === 'history_list') {
        return { current: 3, entries: [{ version: 3, title: 'Doc', createdAt: 30 }] };
      }
      if (cmd === 'history_get') {
        return { version: 1, title: 'Doc', body: 'old body', createdAt: 10 };
      }
      return { ...SNIPPET, version: 4 };
    });

    const history = await historyList('s-1', 100, 0);
    const entry = await historyGet('s-1', 1);
    const restored = await historyRestore('s-1', 1);
    expect(captured['history_list']).toEqual({ id: 's-1', limit: 100, offset: 0 });
    expect(captured['history_get']).toEqual({ id: 's-1', version: 1 });
    expect(captured['history_restore']).toEqual({ id: 's-1', version: 1 });
    expect(history.current).toBe(3);
    expect(history.entries[0]?.version).toBe(3);
    expect(entry.body).toBe('old body');
    expect(restored.version).toBe(4);
  });

  it('sends app-rule list/create/update/delete with the wire contract', async () => {
    const captured: Record<string, Record<string, unknown>> = {};
    const rule = {
      id: 'r-1',
      snippetId: 's-1',
      snippetTitle: 'Sig',
      platform: 'macos',
      appIdentifier: 'com.google.Chrome',
      ruleType: 'disable',
    };
    mockIPC((cmd, args) => {
      captured[cmd] = args as Record<string, unknown>;
      if (cmd === 'app_rule_list') return [rule];
      if (cmd === 'app_rule_delete') return undefined;
      return rule;
    });

    const listed = await appRuleList(100, 0);
    await appRuleCreate({
      snippetId: 's-1',
      appIdentifier: 'com.google.Chrome',
      ruleType: 'disable',
    });
    await appRuleUpdate('r-1', { appIdentifier: 'com.apple.Safari', ruleType: 'show_only' });
    await appRuleDelete('r-1');
    expect(captured['app_rule_list']).toEqual({ limit: 100, offset: 0 });
    expect(captured['app_rule_create']).toEqual({
      input: { snippetId: 's-1', appIdentifier: 'com.google.Chrome', ruleType: 'disable' },
    });
    expect(captured['app_rule_update']).toEqual({
      id: 'r-1',
      input: { appIdentifier: 'com.apple.Safari', ruleType: 'show_only' },
    });
    expect(captured['app_rule_delete']).toEqual({ id: 'r-1' });
    expect(listed[0]?.ruleType).toBe('disable');
  });

  it('sends the panel results request and reads the hidden count', async () => {
    let captured: Record<string, unknown> | undefined;
    mockIPC((cmd, args) => {
      if (cmd === 'panel_results') captured = args as Record<string, unknown>;
      return { rows: [SNIPPET], hiddenByRules: 2 };
    });

    const page = await panelResults('sig', 20);
    expect(captured).toEqual({ query: 'sig', limit: 20 });
    expect(page.rows).toHaveLength(1);
    expect(page.hiddenByRules).toBe(2);
  });

  it('sends the onboarding status/complete and clipboard reads with the wire contract', async () => {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === 'onboarding_status') return { completed: false };
      if (cmd === 'clipboard_read_text') return 'docker logs -f api';
      return undefined;
    });

    const status = await onboardingStatus();
    expect(status.completed).toBe(false);
    await onboardingComplete();
    const seed = await clipboardReadText();
    expect(seed).toBe('docker logs -f api');
    expect(calls).toEqual(['onboarding_status', 'onboarding_complete', 'clipboard_read_text']);
  });

  it('sends the keyboard settings jump with the wire contract and no payload', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return undefined;
    });

    await openKeyboardSettings();
    expect(seen).toHaveLength(1);
    expect(seen[0]?.cmd).toBe('open_keyboard_settings');
    expect(seen[0]?.args).toEqual({});
  });

  it('sends the generic import request with format and content', async () => {
    let captured: Record<string, unknown> | undefined;
    mockIPC((cmd, args) => {
      if (cmd === 'snippets_import') captured = args as Record<string, unknown>;
      return {
        imported: 2,
        conflicts: [':taken'],
        skipped: [{ label: 'row 3', reason: 'row has no content' }],
      };
    });
    const report = await snippetsImport('csv', 'content\nhello\n');
    expect(captured).toEqual({ format: 'csv', content: 'content\nhello\n' });
    expect(report.imported).toBe(2);
    expect(report.conflicts).toEqual([':taken']);
    expect(report.skipped[0]?.label).toBe('row 3');
  });

  it('sends template save/preview with the wire contract', async () => {
    const captured: Record<string, Record<string, unknown>> = {};
    mockIPC((cmd, args) => {
      captured[cmd] = args as Record<string, unknown>;
      if (cmd === 'template_save_fields') return [];
      if (cmd === 'template_preview') return 'prod/‹module›';
      if (cmd === 'template_variables') return ['module'];
      return [];
    });

    await templateVariables('{{module}}');
    await templateFields('s-1');
    await templateSaveFields('s-1', [
      {
        id: '',
        name: 'module',
        label: 'Module',
        fieldType: 'single_line_text',
        defaultValue: 'auth-service',
        options: [],
        validation: null,
        isRequired: true,
        sortOrder: 0,
        platformOverrides: null,
      },
    ]);
    const preview = await templatePreview('{{env}}/{{module}}', [], { env: 'prod' });

    expect(captured['template_variables']).toEqual({ body: '{{module}}' });
    expect(captured['template_fields']).toEqual({ snippetId: 's-1' });
    expect(captured['template_save_fields']?.['snippetId']).toBe('s-1');
    expect(captured['template_preview']).toEqual({
      body: '{{env}}/{{module}}',
      fields: [],
      values: { env: 'prod' },
    });
    expect(preview).toBe('prod/‹module›');
  });

  it('sends template render and panel template insert with the wire contract', async () => {
    const captured: Record<string, Record<string, unknown>> = {};
    mockIPC((cmd, args) => {
      captured[cmd] = args as Record<string, unknown>;
      if (cmd === 'template_render') return '### auth';
      return undefined;
    });

    const text = await templateRender('s-1', { module: 'auth' });
    await panelInsertTemplate('s-1', { module: 'auth' });

    expect(text).toBe('### auth');
    expect(captured['template_render']).toEqual({ snippetId: 's-1', values: { module: 'auth' } });
    expect(captured['panel_insert_template']).toEqual({
      id: 's-1',
      values: { module: 'auth' },
      method: undefined,
    });
  });

  it('sends the mobile bootstrap request with no payload', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return { schemaVersion: 9, snippetTotal: 3 };
    });

    const info = await mobileBootstrap();
    expect(info).toEqual({ schemaVersion: 9, snippetTotal: 3 });
    expect(seen[0]?.cmd).toBe('mobile_bootstrap');
  });

  it('sends the snapshot refresh request with no payload', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return undefined;
    });

    await snapshotRefresh();
    expect(seen[0]?.cmd).toBe('snapshot_refresh');
    expect(seen[0]?.args).toEqual({});
  });

  it('maps the browser-integration command family onto its wire names', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return cmd === 'browser_integration_status'
        ? { enabled: true, hostInstalled: true }
        : undefined;
    });

    await expect(browserIntegrationStatus()).resolves.toEqual({
      enabled: true,
      hostInstalled: true,
    });
    await browserIntegrationEnable();
    await browserIntegrationDisable();
    await browserIntegrationSync();
    expect(seen.map((entry) => entry.cmd)).toEqual([
      'browser_integration_status',
      'browser_integration_enable',
      'browser_integration_disable',
      'browser_integration_sync',
    ]);
    expect(seen.every((entry) => JSON.stringify(entry.args) === '{}')).toBe(true);
  });

  it('maps the semantic command family onto its wire names', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      if (cmd === 'semantic_status') {
        return {
          modelPresent: false,
          downloading: false,
          downloadReceived: 0,
          downloadTotal: 1,
          downloadFailed: false,
          embeddedCount: 0,
          pendingCount: 0,
          modelId: 'm',
        };
      }
      return cmd === 'search_library_deep' ? [] : undefined;
    });

    await expect(semanticStatus()).resolves.toMatchObject({ modelPresent: false });
    await semanticModelDownload();
    await semanticModelDelete();
    await semanticSync();
    await expect(searchLibraryDeep('docker', 50)).resolves.toEqual([]);
    expect(seen.map((entry) => entry.cmd)).toEqual([
      'semantic_status',
      'semantic_model_download',
      'semantic_model_delete',
      'semantic_sync',
      'search_library_deep',
    ]);
    expect(seen[4]?.args).toEqual({ query: 'docker', limit: 50 });
  });

  it('drains the share inbox and resolves with the ingested count', async () => {
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return 2;
    });

    const count = await shareInboxIngest();
    expect(count).toBe(2);
    expect(seen[0]?.cmd).toBe('share_inbox_ingest');
    expect(seen[0]?.args).toEqual({});
  });

  it('surfaces a permission_denied rejection as a typed IpcError', async () => {
    mockIPC(() => {
      throw { code: 'permission_denied', message: 'accessibility permission not granted' };
    });
    const error = await injectSnippet('s-1').catch((e: unknown) => e);
    expect(error).toBeInstanceOf(IpcError);
    expect((error as IpcError).code).toBe('permission_denied');
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
