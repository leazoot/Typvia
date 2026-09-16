// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { UiPrefsProvider } from '@typvia/ui';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { EspansoProvider } from '../../espanso/espanso-context';
import { UndoProvider } from '../../workspace/undo';
import { SettingsPage } from './settings-page';

const espansoStatus = vi.fn();
const espansoDisable = vi.fn();
const espansoCoexistence = vi.fn();
const espansoImport = vi.fn();
const aiProviderList = vi.fn();
const aiProviderSave = vi.fn();
const backupExportFile = vi.fn();
const semanticModelDownload = vi.fn();
const appRuleList = vi.fn();
const appRuleCreate = vi.fn();
const appRuleDelete = vi.fn();
const appRuleUpdate = vi.fn();
const searchLibrary = vi.fn();

vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.1.0') }));

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  espansoStatus: () => espansoStatus() as Promise<unknown>,
  espansoSync: () => Promise.resolve({ written: 0 }),
  espansoDisable: () => espansoDisable() as Promise<unknown>,
  espansoCoexistence: (choice: string) => espansoCoexistence(choice) as Promise<unknown>,
  espansoImport: (content: string) => espansoImport(content) as Promise<unknown>,
  snippetsImport: () => Promise.resolve({ imported: 0, conflicts: [], skipped: [] }),
  aiProviderList: () => aiProviderList() as Promise<unknown>,
  aiProviderSave: (input: unknown) => aiProviderSave(input) as Promise<unknown>,
  aiApiKeySet: () => Promise.resolve(),
  aiEgressLogList: () => Promise.resolve({ entries: [], total: 0 }),
  backupExportFile: (passphrase: string) => backupExportFile(passphrase) as Promise<unknown>,
  backupRestore: () =>
    Promise.resolve({ snippets: 0, folders: 0, tags: 0, versions: 0, vaultRestored: false }),
  libraryCounts: () => Promise.resolve({ total: 3, trash: 0, favorites: 1, recent: 2 }),
  listSnippets: () => Promise.resolve([]),
  syncStatus: () =>
    Promise.resolve({
      available: true,
      configured: false,
      enabled: false,
      serverUrl: null,
      accountId: null,
      deviceId: 'dev-1',
      deviceName: 'Test Mac',
      keyGeneration: 1,
      pendingBacklog: 0,
      conflictCount: 0,
      lastSyncAt: null,
      vaultReady: false,
      vaultUnlocked: false,
      recoveryExportedAt: null,
      recoveryCatchupPending: false,
      transportKind: 'server',
    }),
  syncDevices: () => Promise.resolve([]),
  semanticStatus: () =>
    Promise.resolve({
      modelPresent: false,
      downloading: false,
      downloadReceived: 0,
      downloadTotal: 0,
      downloadFailed: false,
      embeddedCount: 0,
      pendingCount: 0,
      modelId: 'e5-small',
    }),
  semanticModelDownload: () => semanticModelDownload() as Promise<unknown>,
  searchLibrary: (query: string) => searchLibrary(query) as Promise<unknown>,
  searchLibraryDeep: () => Promise.resolve([]),
  browserIntegrationStatus: () => Promise.resolve({ enabled: false, hostInstalled: false }),
  appRuleList: () => appRuleList() as Promise<unknown>,
  appRuleCreate: (input: unknown) => appRuleCreate(input) as Promise<unknown>,
  appRuleDelete: (id: string) => appRuleDelete(id) as Promise<unknown>,
  appRuleUpdate: (id: string, input: unknown) => appRuleUpdate(id, input) as Promise<unknown>,
}));

const engineRunning = {
  state: 'running',
  version: '2.4.0',
  configPath: '/x/engine/config/match/typvia.yml',
  enabled: true,
  triggerCount: 2,
  coexistenceChoice: null,
};

const rule = {
  id: 'r-1',
  snippetId: 's-1',
  snippetTitle: 'Sig',
  platform: 'macos',
  appIdentifier: 'com.google.Chrome',
  ruleType: 'disable',
};

function renderPage() {
  return render(
    <UiPrefsProvider>
      <MemoryRouter>
        <UndoProvider>
          <EspansoProvider>
            <SettingsPage />
          </EspansoProvider>
        </UndoProvider>
      </MemoryRouter>
    </UiPrefsProvider>,
  );
}

beforeEach(() => {
  localStorage.setItem('tv.ui.locale', 'en');
  espansoStatus.mockResolvedValue(engineRunning);
  espansoDisable.mockResolvedValue({ written: 0 });
  espansoCoexistence.mockResolvedValue(undefined);
  aiProviderList.mockResolvedValue([]);
  appRuleList.mockResolvedValue([]);
  searchLibrary.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

describe('SettingsPage — one page', () => {
  it('lays every group out on one page, with no group folded away', async () => {
    renderPage();
    for (const name of [
      'General',
      'Insertion',
      'Sync',
      'Shortcuts',
      'Import & export',
      'Language',
      'Appearance',
      'Semantic search',
      'AI provider',
      'Per-app visibility',
    ]) {
      expect(await screen.findByRole('heading', { name })).toBeDefined();
    }
    expect(await screen.findByText('Running · 2 triggers')).toBeDefined();
    expect(screen.queryByRole('button', { expanded: false })).toBeNull();
  });

  it('switches the interface language the moment it is chosen', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('radio', { name: '中文' }));
    expect(localStorage.getItem('tv.ui.locale')).toBe('zh');
    expect(await screen.findByRole('heading', { name: '语言' })).toBeDefined();
  });

  it('keeps the insertion method and says what it means', async () => {
    renderPage();
    expect(await screen.findByText(/pastes it in one go/)).toBeDefined();
    fireEvent.click(screen.getByRole('radio', { name: 'Type it out' }));
    expect(localStorage.getItem('tv.ui.insertMethod')).toBe('keystrokes');
    expect(await screen.findByText(/one character at a time/)).toBeDefined();
  });
});

describe('SettingsPage — expansion engine', () => {
  it('offers both answers on a conflict and never takes over by itself', async () => {
    espansoStatus.mockResolvedValue({ ...engineRunning, state: 'conflict' });
    renderPage();
    expect(await screen.findByText(/Two engines would fight/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Keep my Espanso' })).toBeDefined();
    expect(espansoCoexistence).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Use Typvia’s engine' }));
    await waitFor(() => expect(espansoCoexistence).toHaveBeenCalledWith('takeover'));
  });

  it('turns the engine off from its row', async () => {
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Turn the engine off' }));
    await waitFor(() => expect(espansoDisable).toHaveBeenCalledTimes(1));
  });
});

describe('SettingsPage — import and export', () => {
  it('infers the format from the chosen file and reports honestly', async () => {
    espansoImport.mockResolvedValue({
      imported: 2,
      conflicts: [],
      skipped: [{ trigger: ':form', reason: 'uses forms' }],
    });
    renderPage();
    const input = await screen.findByLabelText('Import file');
    fireEvent.change(input, {
      target: { files: [new File(['matches: []'], 'base.yml', { type: 'text/yaml' })] },
    });
    const espanso = await screen.findByRole('radio', { name: 'Espanso' });
    expect(espanso.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'Import' }));
    await waitFor(() => expect(espansoImport).toHaveBeenCalledWith('matches: []'));
    expect(await screen.findByText(/Imported 2/)).toBeDefined();
    expect(screen.getByText(/:form — uses forms/)).toBeDefined();
  });

  it('asks for the passphrase on demand and holds the export until it is long enough', async () => {
    backupExportFile.mockResolvedValue('Typvia-backup-1.json');
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Export all snippets' }));
    const field = screen.getByLabelText('Backup passphrase');
    fireEvent.change(field, { target: { value: 'short' } });
    const exportButton = screen.getByRole('button', { name: 'Export' });
    expect(exportButton.hasAttribute('disabled')).toBe(true);
    fireEvent.change(field, { target: { value: 'long enough passphrase' } });
    fireEvent.click(exportButton);
    await waitFor(() => expect(backupExportFile).toHaveBeenCalledWith('long enough passphrase'));
    expect(await screen.findByText(/Saved as Typvia-backup-1\.json/)).toBeDefined();
  });
});

describe('SettingsPage — semantic search', () => {
  it('downloads the model when asked', async () => {
    semanticModelDownload.mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Download the model' }));
    await waitFor(() => expect(semanticModelDownload).toHaveBeenCalledTimes(1));
  });
});

describe('SettingsPage — AI provider', () => {
  it('connects a new provider from the inline lines', async () => {
    aiProviderSave.mockResolvedValue({ id: 'p-1' });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Connect a provider' }));
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Local Ollama' } });
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'llama3' } });
    fireEvent.click(screen.getByRole('button', { name: 'Connect' }));
    await waitFor(() =>
      expect(aiProviderSave).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'Local Ollama', model: 'llama3', kind: 'ollama' }),
      ),
    );
  });

  it('shows the current provider with a key that is never read back', async () => {
    aiProviderList.mockResolvedValue([
      {
        id: 'p-1',
        name: 'aggre',
        kind: 'openai_compatible',
        baseUrl: 'https://api.example.test/v1',
        model: 'gpt',
        timeoutMs: 30000,
        hasApiKey: true,
      },
    ]);
    renderPage();
    expect(await screen.findByText('aggre · OpenAI-compatible')).toBeDefined();
    expect(screen.getByText('Kept in the system keychain')).toBeDefined();
    expect(screen.queryByRole('button', { name: /clear the log/i })).toBeNull();
  });
});

describe('SettingsPage — per-app visibility', () => {
  it('says every app sees every snippet while there are no rules', async () => {
    renderPage();
    expect(await screen.findByText('Every app can use all 3 snippets.')).toBeDefined();
  });

  it('lists a rule in one line: app, effect, snippet', async () => {
    appRuleList.mockResolvedValue([rule]);
    renderPage();
    const list = within(await screen.findByRole('list', { name: 'App rules' }));
    expect(list.getByText('Sig')).toBeDefined();
    expect(list.getByText('com.google.Chrome')).toBeDefined();
    expect(list.getByText('Hide here')).toBeDefined();
  });

  it('adds a rule for a picked snippet', async () => {
    searchLibrary.mockResolvedValue([{ id: 's-9', title: 'Address' }]);
    appRuleCreate.mockResolvedValue({ ...rule, id: 'r-9' });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Add the first rule' }));
    fireEvent.change(screen.getByLabelText('Find a snippet'), { target: { value: 'addr' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Address' }));
    fireEvent.change(screen.getByLabelText('App identifier'), {
      target: { value: 'com.tinyspeck.slackmacgap' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add the rule' }));
    await waitFor(() =>
      expect(appRuleCreate).toHaveBeenCalledWith({
        snippetId: 's-9',
        appIdentifier: 'com.tinyspeck.slackmacgap',
        ruleType: 'disable',
      }),
    );
  });

  it('says a refused rule was not added, without pretending', async () => {
    searchLibrary.mockResolvedValue([{ id: 's-9', title: 'Address' }]);
    appRuleCreate.mockRejectedValue({ code: 'conflict', message: 'this rule already exists' });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Add the first rule' }));
    fireEvent.change(screen.getByLabelText('Find a snippet'), { target: { value: 'addr' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Address' }));
    fireEvent.change(screen.getByLabelText('App identifier'), { target: { value: 'com.x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add the rule' }));
    expect(await screen.findByText(/^No rule was added — /)).toBeDefined();
  });

  it('removes a rule at once and offers it back', async () => {
    appRuleList.mockResolvedValue([rule]);
    appRuleDelete.mockResolvedValue(undefined);
    renderPage();
    const list = within(await screen.findByRole('list', { name: 'App rules' }));
    fireEvent.click(list.getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(appRuleDelete).toHaveBeenCalledWith('r-1'));
    expect(await screen.findByText('Removed the rule for com.google.Chrome.')).toBeDefined();
  });

  it('changes a rule in place', async () => {
    appRuleList.mockResolvedValue([rule]);
    appRuleUpdate.mockResolvedValue(rule);
    renderPage();
    const list = within(await screen.findByRole('list', { name: 'App rules' }));
    fireEvent.click(list.getByRole('button', { name: 'Change' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Show only here' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(appRuleUpdate).toHaveBeenCalledWith('r-1', {
        appIdentifier: 'com.google.Chrome',
        ruleType: 'show_only',
      }),
    );
  });
});
