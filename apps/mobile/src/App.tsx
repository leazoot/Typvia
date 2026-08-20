import {
  createSnapshotRefresher,
  ipcErrorCopy,
  mobileBootstrap,
  onboardingStatus,
  snapshotRefresh,
} from '@typvia/shared';
import type { MobileBootstrap, Snippet } from '@typvia/shared';
import { getCurrent as getInitialDeepLink, onOpenUrl } from '@tauri-apps/plugin-deep-link';
import {
  Caret,
  HomeNavIcon,
  LibraryNavIcon,
  SettingsNavIcon,
  TabBar,
  Toast,
  VaultNavIcon,
  designTokens,
  useLocale,
  useTr,
} from '@typvia/ui';
import type { TabBarItem } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { DetailPage } from './pages/detail';
import { HomePage } from './pages/home';
import { LibraryPage } from './pages/library';
import { OnboardingPage } from './pages/onboarding';
import { SearchPage } from './pages/search';
import { SettingsPage } from './pages/settings';
import { SnippetEditorPage } from './pages/editor';
import { VaultPage } from './pages/vault';
import { createDeepLinkHandler } from './deep-link';
import { createShareIngestor } from './share-ingest';

import { createSyncTrigger } from './sync-trigger';
import './theme.css';
import './app.css';

const TABS = [
  { key: 'home', labelEn: 'Home', labelCn: '首页', icon: <HomeNavIcon /> },
  { key: 'library', labelEn: 'Library', labelCn: '资料库', icon: <LibraryNavIcon /> },
  { key: 'vault', labelEn: 'Vault', labelCn: '保险库', icon: <VaultNavIcon /> },
  { key: 'settings', labelEn: 'Settings', labelCn: '设置', icon: <SettingsNavIcon /> },
] as const;

type EditorState = { mode: 'create' } | { mode: 'edit'; snippet: Snippet };

/**
 * Mobile app shell: boot handshake, bottom tab bar and page switching — no
 * page transitions (motion rule). The boot smoke keeps the shared semantics:
 * a text loading state while the handshake is in flight, and a
 * data-untouched error screen when the core cannot open.
 *
 * Create/edit is a full-screen page (forms are pages) replacing the tab
 * shell; closing it remounts the tab page, which refetches — that remount is
 * the list-refresh signal after a save.
 */
export function App() {
  const tr = useTr();
  const locale = useLocale();
  const tabs: readonly TabBarItem[] = TABS.map((tab) => ({
    key: tab.key,
    label: locale === 'zh' ? tab.labelCn : tab.labelEn,
    icon: tab.icon,
  }));
  const [info, setInfo] = useState<MobileBootstrap | null>(null);
  const [error, setError] = useState<string | null>(null);
  // null = first-run status pending (nothing rendered — the flow decision is
  // a cut, not a flash of shell). A failed read fails open into the shell:
  // the app staying usable matters more than re-offering setup. Mirrors the
  // desktop App gating.
  const [onboarded, setOnboarded] = useState<boolean | null>(null);
  const [tab, setTab] = useState('home');
  // Fullscreen layers over the tab shell, closest on top:
  // editor > detail > search > tabs.
  const [search, setSearch] = useState(false);
  const [detail, setDetail] = useState<Snippet | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [savedToast, setSavedToast] = useState<string | null>(null);
  // Bumped on every library write; keys the list pages so they refetch —
  // the sheet no longer unmounts the shell, so remount-on-close is gone.
  const [epoch, setEpoch] = useState(0);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const snapshotRefresher = useRef(createSnapshotRefresher(snapshotRefresh));

  // Deep-link channel: cold-start URL plus live opens both
  // feed the same validated router; plugin failures (e.g. web dev shell)
  // leave the app exactly as launched.
  useEffect(() => {
    const handle = createDeepLinkHandler({
      openSearch: () => {
        setEditor(null);
        setDetail(null);
        setTab('home');
        setSearch(true);
      },
      openSnippet: (snippet) => {
        setEditor(null);
        setDetail(snippet);
      },
    });
    getInitialDeepLink()
      .then((urls) => (urls === null ? undefined : handle(urls)))
      .catch(() => undefined);
    const unlisten = onOpenUrl((urls) => void handle(urls)).catch(() => null);
    return () => {
      void unlisten.then((stop) => stop?.());
    };
  }, []);

  useEffect(() => {
    onboardingStatus()
      .then((status) => {
        setOnboarded(status.completed);
      })
      .catch(() => {
        setOnboarded(true);
      });
  }, []);

  useEffect(() => {
    let cancelled = false;
    mobileBootstrap()
      .then((result) => {
        if (!cancelled) setInfo(result);
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setError(
          caught instanceof Error
            ? tr(...ipcErrorCopy(caught))
            : tr('the core did not answer', '核心没有响应'),
        );
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(
    () => () => {
      if (toastTimer.current !== null) clearTimeout(toastTimer.current);
      snapshotRefresher.current.cancel();
    },
    [],
  );

  // Toast + refresh after any write that changed the library (an editor
  // save, an ingested share): the boot totals are re-read so Home's count
  // and empty state stay honest — on failure the previous totals stand, the
  // lists refetch on remount regardless and the write already succeeded —
  // and the keyboard snapshot is kept in step with the library.
  const noteLibraryWrite = (message: string) => {
    setEpoch((n) => n + 1);
    setSavedToast(message);
    if (toastTimer.current !== null) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => {
      setSavedToast(null);
    }, designTokens.motion.duration.toastLife);
    mobileBootstrap().then(
      (next) => {
        setInfo(next);
      },
      () => undefined,
    );
    snapshotRefresher.current.notifyMutation();
  };
  const noteLibraryWriteRef = useRef(noteLibraryWrite);
  noteLibraryWriteRef.current = noteLibraryWrite;
  // Same ref pattern for the string picker: the ingest effect must not
  // re-subscribe on a language switch, but its toast should use the current
  // language when it fires.
  const trRef = useRef(tr);
  trRef.current = tr;

  // Share-extension inbox drain: once when the shell is ready,
  // then on every return to the foreground. Positive counts toast with the
  // real number.
  const shellReady = onboarded === true && error === null && info !== null;
  useEffect(() => {
    if (!shellReady) return;
    const ingestor = createShareIngestor((count) => {
      noteLibraryWriteRef.current(
        trRef.current(`Saved from share · ${count}`, `已保存分享内容 · ${count}`),
      );
    });
    ingestor.start();
    return () => {
      ingestor.stop();
    };
  }, [shellReady]);

  // Foreground sync: the host already runs a round at launch and
  // on the native Resumed event; this covers the WebView's own return to
  // visibility. A round that cannot reach the server is the offline state,
  // so nothing here surfaces an error.
  useEffect(() => {
    if (!shellReady) return;
    const trigger = createSyncTrigger();
    trigger.start();
    return () => {
      trigger.stop();
    };
  }, [shellReady]);

  const handleSaved = (saved: Snippet) => {
    setEditor(null);
    // An edit opened from the detail screen lands back on fresh data.
    setDetail((open) => (open === null ? null : saved));
    const trigger = saved.trigger;
    noteLibraryWrite(
      trigger === null || trigger === ''
        ? tr('Saved', '已保存')
        : tr(`Saved · ${trigger}`, `已保存 · ${trigger}`),
    );
  };

  const openDetail = (snippet: Snippet) => {
    setDetail(snippet);
  };

  if (onboarded === null) return null;

  if (!onboarded) {
    return (
      <OnboardingPage
        onDone={() => {
          setOnboarded(true);
          // Re-read the boot totals: a snippet may have been saved during
          // setup; on failure the pre-setup totals stand.
          mobileBootstrap().then(
            (next) => {
              setInfo(next);
            },
            () => undefined,
          );
        }}
        onSnippetSaved={() => {
          snapshotRefresher.current.notifyMutation();
        }}
      />
    );
  }

  if (error !== null) {
    return (
      <main className="tv-app-boot">
        <p role="status">
          {tr(
            `Your data is untouched on this device — the app could not open it: ${error}`,
            `你的数据在本机完好未动 —— 应用暂时无法打开它：${error}`,
          )}
        </p>
      </main>
    );
  }

  if (info === null) {
    return (
      <main className="tv-app-boot">
        <Caret height={16} />
        <p role="status">{tr('Opening your library…', '正在打开你的片段库…')}</p>
      </main>
    );
  }

  // The create/edit sheet floats over whatever layer is open — a dimmed
  // shell stays visible behind the sheet — so the base layer keeps rendering
  // underneath.
  const editorSheet =
    editor !== null ? (
      <SnippetEditorPage
        snippet={editor.mode === 'edit' ? editor.snippet : undefined}
        onCancel={() => {
          setEditor(null);
        }}
        onSaved={handleSaved}
      />
    ) : null;

  if (detail !== null) {
    return (
      <div className="tv-app">
        <div className="tv-app-page">
          <DetailPage
            snippet={detail}
            onBack={() => {
              setDetail(null);
            }}
            onEdit={(snippet) => {
              setEditor({ mode: 'edit', snippet });
            }}
            onDeleted={() => {
              setDetail(null);
              setSearch(false);
              noteLibraryWrite(tr('Deleted', '已删除'));
            }}
          />
        </div>
        {editorSheet}
        {savedToast !== null && (
          <div className="tv-mobile-toast">
            <Toast message={savedToast} assertive />
          </div>
        )}
      </div>
    );
  }

  if (search) {
    return (
      <div className="tv-app">
        <div className="tv-app-page">
          <SearchPage
            onCancel={() => {
              setSearch(false);
            }}
            onOpen={openDetail}
          />
        </div>
        {editorSheet}
      </div>
    );
  }

  return (
    <div className="tv-app">
      <div className="tv-app-page" key={epoch}>
        {tab === 'settings' ? (
          <SettingsPage />
        ) : tab === 'vault' ? (
          <VaultPage />
        ) : tab === 'library' ? (
          <LibraryPage onOpen={openDetail} />
        ) : (
          <HomePage
            total={info.snippetTotal}
            onOpenSearch={() => {
              setSearch(true);
            }}
            onOpen={openDetail}
          />
        )}
      </div>
      <TabBar
        items={tabs}
        activeKey={tab}
        onSelect={(key) => {
          setTab(key);
        }}
        onCreate={() => {
          setEditor({ mode: 'create' });
        }}
      />
      {editorSheet}
      {savedToast !== null && (
        <div className="tv-mobile-toast">
          <Toast message={savedToast} assertive />
        </div>
      )}
    </div>
  );
}
