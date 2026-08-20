/**
 * Settings — the redesigned preferences surface: State → Peek → Expand →
 * Advanced. A restrained header, an ambient status line, and eight quiet
 * summary rows that fold open in place. No sidebar, no cards, no permanently
 * exposed low-frequency actions.
 */
import { aiProviderList, libraryCounts, syncStatus } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useEspanso } from '../../espanso/espanso-context';
import { SettingsPalette, type PaletteEntry } from './prefs/palette';
import { Dot } from './prefs/pref-kit';
import { PrefSections, PrefsProvider } from './prefs/pref-section';
import { AiPref } from './prefs/panel-ai';
import { BrowserPref } from './prefs/panel-browser';
import { DataPref } from './prefs/panel-data';
import { EnginePref } from './prefs/panel-engine';
import { GeneralPref } from './prefs/panel-general';
import { ImportPref } from './prefs/panel-import';
import { SemanticPref } from './prefs/panel-semantic';
import { SyncPref } from './prefs/panel-sync';
import './prefs/prefs.css';
import './settings.css';

export function SettingsPage() {
  return (
    <PrefsProvider>
      <SettingsSurface />
    </PrefsProvider>
  );
}

function SettingsSurface() {
  const tr = useTr();
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if (event.key === 'Escape') setPaletteOpen(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  const paletteEntries: PaletteEntry[] = [
    { group: tr('General', '通用'), label: tr('Language', '语言'), sectionId: 'general' },
    { group: tr('General', '通用'), label: tr('Appearance', '外观'), sectionId: 'general' },
    {
      group: tr('Expansion engine', '展开引擎'),
      label: tr('Engine status', '引擎状态'),
      sectionId: 'engine',
    },
    {
      group: tr('Expansion engine', '展开引擎'),
      label: tr('Config file', '配置文件'),
      sectionId: 'engine',
    },
    {
      group: tr('Semantic search', '语义搜索'),
      label: tr('Local model', '本地模型'),
      sectionId: 'semantic',
    },
    {
      group: tr('Browser extension', '浏览器扩展'),
      label: tr('Browser extension', '浏览器扩展'),
      sectionId: 'browser',
    },
    { group: tr('Sync', '同步'), label: tr('Sync server', '同步服务器'), sectionId: 'sync' },
    { group: tr('Sync', '同步'), label: tr('Paired devices', '配对设备'), sectionId: 'sync' },
    { group: 'AI', label: 'AI Provider', sectionId: 'ai' },
    { group: 'AI', label: 'API Key', sectionId: 'ai' },
    { group: 'AI', label: tr('Outbound activity', '出站活动'), sectionId: 'ai' },
    {
      group: tr('Import', '导入'),
      label: tr('Import from a file', '从文件导入'),
      sectionId: 'import',
    },
    {
      group: tr('Data & privacy', '数据与隐私'),
      label: tr('Encrypted backup', '加密备份'),
      sectionId: 'data',
    },
    {
      group: tr('Data & privacy', '数据与隐私'),
      label: tr('Restore from a backup', '从备份恢复'),
      sectionId: 'data',
    },
    {
      group: tr('Data & privacy', '数据与隐私'),
      label: tr('App visibility rules', '应用可见性规则'),
      sectionId: 'data',
    },
  ];

  return (
    <main className="tvp">
      <header className="tvp-head">
        <div>
          <h1>{tr('Preferences', '设置')}</h1>
          <p className="tvp-tagline">{tr('Make Typvia yours.', '让 Typvia 按你的方式工作。')}</p>
        </div>
        <button
          type="button"
          className="tvp-search"
          aria-label={tr('Search settings', '搜索设置')}
          onClick={() => setPaletteOpen(true)}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <circle cx="7" cy="7" r="4.6" />
            <path d="M10.4 10.4 14 14" />
          </svg>
          <span className="tvp-search-reveal">
            <span>{tr('Search settings', '搜索设置')}</span>
            <kbd>⌘K</kbd>
          </span>
        </button>
      </header>

      <AmbientStatus />

      <PrefSections>
        <GeneralPref />
        <EnginePref />
        <SemanticPref />
        <BrowserPref />
        <SyncPref />
        <AiPref />
        <ImportPref />
        <DataPref />
      </PrefSections>

      <SettingsPalette
        entries={paletteEntries}
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
      />
    </main>
  );
}

/** One quiet line of ambient truth: is Typvia working right now? */
function AmbientStatus() {
  const tr = useTr();
  const { status: engine } = useEspanso();
  const [snippets, setSnippets] = useState<number | null>(null);
  const [sync, setSync] = useState<{ enabled: boolean; pending: number } | null>(null);
  const [provider, setProvider] = useState<string | null>(null);

  useEffect(() => {
    libraryCounts()
      .then((counts) => setSnippets(counts.total))
      .catch(() => undefined);
    syncStatus()
      .then((status) =>
        setSync(
          status.configured ? { enabled: status.enabled, pending: status.pendingBacklog } : null,
        ),
      )
      .catch(() => undefined);
    aiProviderList()
      .then((providers) => setProvider(providers[0]?.name ?? null))
      .catch(() => undefined);
  }, []);

  const engineOk = engine?.state === 'running';
  const engineQuietStates = new Set(['off', 'standing_aside', 'unavailable']);

  return (
    <div className="tvp-ambient">
      <span className="tvp-lead">
        {engineOk ? (
          <>
            <Dot kind="ok" />
            {tr('Typvia is working', 'Typvia 正在工作')}
          </>
        ) : engine && !engineQuietStates.has(engine.state) ? (
          <>
            <Dot kind="warn" />
            {tr('Expansion is paused — everything else works', '展开已暂停——其余一切照常')}
          </>
        ) : (
          <>
            <Dot kind="idle" />
            {tr('Typvia is ready', 'Typvia 就绪')}
          </>
        )}
      </span>
      {snippets !== null && (
        <span>
          {tr(
            `${String(snippets)} snippet${snippets === 1 ? '' : 's'}`,
            `${String(snippets)} 个片段`,
          )}
        </span>
      )}
      {sync !== null && (
        <span>
          {!sync.enabled
            ? tr('Sync paused', '同步已暂停')
            : sync.pending > 0
              ? tr(`${String(sync.pending)} to sync`, `${String(sync.pending)} 项待同步`)
              : tr('Synced', '已同步')}
        </span>
      )}
      {provider !== null && <span>AI · {provider}</span>}
    </div>
  );
}
