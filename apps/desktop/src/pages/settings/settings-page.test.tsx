// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EspansoProvider } from '../../espanso/espanso-context';
import { SettingsPage } from './settings-page';

const espansoStatus = vi.fn();
const espansoSync = vi.fn();
const espansoDisable = vi.fn();
const espansoCoexistence = vi.fn();
const espansoImport = vi.fn();
const snippetsImport = vi.fn();
const aiProviderList = vi.fn();
const aiProviderSave = vi.fn();
const backupExportFile = vi.fn();

vi.mock('@typvia/shared', async () => ({
  ...(await vi.importActual<object>('@typvia/shared')),
  espansoStatus: () => espansoStatus() as Promise<unknown>,
  espansoSync: () => espansoSync() as Promise<unknown>,
  espansoDisable: () => espansoDisable() as Promise<unknown>,
  espansoCoexistence: (choice: string) => espansoCoexistence(choice) as Promise<unknown>,
  espansoImport: (content: string) => espansoImport(content) as Promise<unknown>,
  snippetsImport: (format: string, content: string) =>
    snippetsImport(format, content) as Promise<unknown>,
  aiProviderList: () => aiProviderList() as Promise<unknown>,
  aiProviderSave: (input: unknown) => aiProviderSave(input) as Promise<unknown>,
  aiApiKeySet: () => Promise.resolve(),
  backupExportFile: (passphrase: string) => backupExportFile(passphrase) as Promise<unknown>,
  // Ambient/status loads on mount; quiet, honest defaults for the shell.
  libraryCounts: () => Promise.resolve({ total: 3, trash: 0, favorites: 1, recent: 2 }),
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
      recoveryCatchUpPending: false,
    }),
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
  browserIntegrationStatus: () => Promise.resolve({ enabled: false, hostInstalled: false }),
  aiEgressLogList: () => Promise.resolve({ entries: [], total: 0 }),
  appRuleList: () => Promise.resolve([]),
  backupRestore: () =>
    Promise.resolve({ snippets: 0, folders: 0, tags: 0, versions: 0, vaultRestored: false }),
}));

const engineRunning = {
  state: 'running',
  version: '2.4.0',
  configPath: '/x/engine/config/match/typvia.yml',
  enabled: true,
  triggerCount: 2,
  coexistenceChoice: null,
};

function renderPage() {
  return render(
    <MemoryRouter>
      <EspansoProvider>
        <SettingsPage />
      </EspansoProvider>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('SettingsPage — peek layer', () => {
  it('shows eight quiet summary rows and the ambient status line', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    renderPage();

    expect(await screen.findByText('Typvia is working')).toBeDefined();
    expect(screen.getByText('3 snippets')).toBeDefined();
    for (const name of [
      'General',
      'Expansion engine',
      'Semantic search',
      'Browser extension',
      'Sync',
      'AI',
      'Import',
      'Data & privacy',
    ]) {
      expect(screen.getByRole('button', { name: new RegExp(name) })).toBeDefined();
    }
    // Peek only: expanded-panel content stays unmounted until a row opens.
    expect(screen.queryByText('Engine running')).toBeNull();
  });

  it('expands a section in place and keeps others closed', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /Expansion engine/ }));
    expect(await screen.findByText('Engine running')).toBeDefined();
    expect(screen.getByText(/Changes apply instantly/)).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: /^General/ }));
    await waitFor(() => expect(screen.queryByText('Engine running')).toBeNull());
    expect(screen.getByRole('group', { name: 'Language' })).toBeDefined();
  });

  it('keeps multiple sections open with an alt-click', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /Expansion engine/ }));
    fireEvent.click(screen.getByRole('button', { name: /^General/ }), { altKey: true });
    expect(await screen.findByText('Engine running')).toBeDefined();
    expect(screen.getByRole('group', { name: 'Language' })).toBeDefined();
  });
});

describe('SettingsPage — engine coexistence', () => {
  it('offers takeover and stand-aside on a conflict, never a silent takeover', async () => {
    espansoStatus.mockResolvedValue({ ...engineRunning, state: 'conflict' });
    aiProviderList.mockResolvedValue([]);
    espansoCoexistence.mockResolvedValue(undefined);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /Expansion engine/ }));
    expect(await screen.findByText(/Two engines would fight/)).toBeDefined();
    expect(screen.getByRole('button', { name: 'Keep my Espanso' })).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Use Typvia’s engine' }));
    await waitFor(() => expect(espansoCoexistence).toHaveBeenCalledWith('takeover'));
  });

  it('turn-off hides in the overflow menu, at the bottom, after a divider', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    espansoDisable.mockResolvedValue({ enabled: false, triggerCount: 0 });
    aiProviderList.mockResolvedValue([]);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /Expansion engine/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Engine actions' }));
    const menu = screen.getByRole('menu');
    // The expanded panel clips its own content while it animates, so the menu
    // must not live inside it (anchored-menu.ts).
    expect(menu.closest('.tvp-panel-clip')).toBeNull();
    const items = within(menu).getAllByRole('menuitem');
    expect(items[items.length - 1]!.textContent).toBe('Turn off');
    fireEvent.click(items[items.length - 1]!);
    await waitFor(() => expect(espansoDisable).toHaveBeenCalledTimes(1));
  });
});

describe('SettingsPage — import', () => {
  it('infers the format from the dropped file and reports honestly', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    espansoImport.mockResolvedValue({
      imported: 2,
      conflicts: [':x'],
      skipped: [{ trigger: ':form', reason: 'uses forms' }],
    });
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /^Import/ }));
    const file = new File(['matches: []'], 'base.yml', { type: 'text/yaml' });
    fireEvent.change(screen.getByLabelText('Import file'), { target: { files: [file] } });
    await screen.findByText(/Ready to import base\.yml/);
    // The .yml drop selected the Espanso format on its own.
    expect(screen.getByRole('button', { name: 'Espanso' }).getAttribute('aria-pressed')).toBe(
      'true',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Import' }));

    await waitFor(() => expect(espansoImport).toHaveBeenCalledWith('matches: []'));
    expect(await screen.findByText(/Imported 2/)).toBeDefined();
    expect(screen.getByText(/:form — uses forms/)).toBeDefined();
  });
});

describe('SettingsPage — command palette', () => {
  it('opens with ⌘K, filters, and lands on the matching section', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    renderPage();
    await screen.findByText('Typvia is working');

    fireEvent.keyDown(document, { key: 'k', metaKey: true });
    const palette = await screen.findByRole('dialog', { name: 'Search settings' });
    fireEvent.change(within(palette).getByPlaceholderText('Search settings…'), {
      target: { value: 'backup' },
    });
    expect(within(palette).queryByRole('button', { name: /Engine status/ })).toBeNull();
    fireEvent.click(within(palette).getByRole('button', { name: /Encrypted backup/ }));

    // The palette closed and the Data & privacy section opened in place.
    await waitFor(() => expect(screen.queryByPlaceholderText('Search settings…')).toBeNull());
    expect(await screen.findByText(/sealed with a passphrase/)).toBeDefined();
  });
});

describe('SettingsPage — data & privacy', () => {
  it('reveals the passphrase field on demand and gates the export on length', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    backupExportFile.mockResolvedValue('Typvia-backup-1.json');
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /Data & privacy/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Export backup' }));
    const pass = await screen.findByLabelText('Backup passphrase');
    const exportButton = screen.getByRole('button', { name: 'Export' });
    fireEvent.change(pass, { target: { value: 'short' } });
    expect(exportButton.hasAttribute('disabled')).toBe(true);
    fireEvent.change(pass, { target: { value: 'long enough passphrase' } });
    fireEvent.click(exportButton);
    await waitFor(() => expect(backupExportFile).toHaveBeenCalledWith('long enough passphrase'));
    expect(await screen.findByText(/Saved as Typvia-backup-1\.json/)).toBeDefined();
  });
});

describe('SettingsPage — AI', () => {
  it('saves a new provider from the inline sheet', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([]);
    aiProviderSave.mockResolvedValue({ id: 'p-new' });
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /^AI/ }));
    fireEvent.click(await screen.findByText('＋ Add provider'));
    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: 'Local Ollama' } });
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'llama3' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));

    await waitFor(() =>
      expect(aiProviderSave).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'Local Ollama', model: 'llama3', kind: 'ollama' }),
      ),
    );
  });

  it('lists providers quietly and opens the inline sheet to add one', async () => {
    espansoStatus.mockResolvedValue(engineRunning);
    aiProviderList.mockResolvedValue([
      {
        id: 'p1',
        name: 'aggre',
        kind: 'openai_compatible',
        baseUrl: 'https://x/v1',
        model: 'gpt',
        timeoutMs: 30000,
        hasApiKey: true,
      },
    ]);
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: /^AI/ }));
    expect(await screen.findByText('aggre · gpt')).toBeDefined();
    expect(screen.getByText(/API key stored in the macOS Keychain/)).toBeDefined();

    fireEvent.click(screen.getByText('＋ Add provider'));
    expect(await screen.findByText('How do you want to connect?')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDefined();
  });
});
