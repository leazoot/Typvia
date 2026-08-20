import { listSnippetPage, syncStatus } from '@typvia/shared';
import type { Snippet, SyncStatus } from '@typvia/shared';
import { Caret, SnippetStripSkeleton, useLocale, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { recencyWord } from '../../components/format';
import { SearchLineButton } from '../../components/search-line';
import { SnippetStripRow } from '../../components/snippet-strip-row';
import './home.css';

const RECENT_LIMIT = 20;
const SKELETON_STRIPS = 4;

interface HomePageProps {
  /** Library total from the boot handshake — drives the empty state. */
  total: number;
  /** The search line is a door: tapping it opens the Search screen. */
  onOpenSearch: () => void;
  /** Row tap opens the snippet detail screen. */
  onOpen: (snippet: Snippet) => void;
}

/** Time-of-day greeting ("Good evening" / 晚上好). */
function greeting(hour: number, tr: (en: string, zh: string) => string): string {
  if (hour < 6) return tr('Up late', '夜深了');
  if (hour < 12) return tr('Good morning', '早上好');
  if (hour < 18) return tr('Good afternoon', '下午好');
  return tr('Good evening', '晚上好');
}

/**
 * Mobile Home: brand caret row, greeting,
 * the "What do you need?" headline, the search line as the single entry
 * point, recent strips, and a status line that states only real facts.
 */
export function HomePage({ total, onOpenSearch, onOpen }: HomePageProps) {
  const tr = useTr();
  const locale = useLocale();
  // null while loading; the 'recent' view is the host's last-used ordering.
  const [recent, setRecent] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [sync, setSync] = useState<SyncStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    setRecent(null);
    setFailed(false);
    listSnippetPage('recent', null, null, RECENT_LIMIT, 0)
      .then((rows) => {
        if (!cancelled) setRecent(rows);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  // The status line claims sync only when sync is really on; a failed read
  // simply leaves the local-facts wording.
  useEffect(() => {
    let cancelled = false;
    syncStatus()
      .then((status) => {
        if (!cancelled) setSync(status);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const statusText =
    sync !== null && sync.enabled && sync.lastSyncAt !== null
      ? tr(
          `${total.toLocaleString('en-US')} snippets · Synced ${recencyWord(sync.lastSyncAt, Date.now(), locale)}`,
          `共 ${total.toLocaleString('en-US')} 条 · ${recencyWord(sync.lastSyncAt, Date.now(), locale)}同步`,
        )
      : tr(
          `${total.toLocaleString('en-US')} snippet${total === 1 ? '' : 's'} on this device`,
          `本机共 ${total.toLocaleString('en-US')} 条片段 · 全部在本地`,
        );

  return (
    <main className="tv-mobile-page tv-mhome">
      <header className="tv-mhome-brand">
        <span className="tv-mhome-brand-bar" />
        <span className="tv-mhome-brand-name">Typvia</span>
      </header>

      <div className="tv-mhome-hello">
        <div className="tv-mhome-greeting">{greeting(new Date().getHours(), tr)}</div>
        <h1 className="tv-mhome-headline">
          {tr('What do you', '需要哪一段')}
          <br />
          {tr('need?', '内容？')}
        </h1>
      </div>

      <div className="tv-mhome-search">
        <SearchLineButton
          placeholder={tr('Search snippets, commands, prompts', '搜索片段、命令、提示词')}
          label={tr('Search snippets', '搜索片段')}
          onPress={onOpenSearch}
        />
      </div>

      {total === 0 ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-figure">
            <span className="tv-mobile-state-slot" />
            <Caret height={16} />
          </div>
          <div className="tv-mobile-state-title">{tr('No snippets yet', '还没有片段')}</div>
          <div className="tv-mobile-state-text">
            {tr(
              'Save your first snippet with the centre button — it is ready everywhere at once.',
              '用中间的新建按钮保存第一条片段，它会立刻在所有地方可用。',
            )}
          </div>
        </div>
      ) : failed ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-title">
            {tr('Your snippets are safe on this device.', '你的片段在这台设备上安然无恙。')}
          </div>
          <div className="tv-mobile-state-text">
            {tr('The recent list failed to load just now.', '最近使用列表刚才未能加载。')}
          </div>
          <button
            type="button"
            className="tv-mobile-state-action"
            onClick={() => {
              setAttempt((n) => n + 1);
            }}
          >
            {tr('Retry', '重试')}
          </button>
        </div>
      ) : (
        <section aria-label={tr('Recently used', '最近使用')} className="tv-mhome-recent">
          <div className="tv-mobile-label">{tr('Recent', '最近')}</div>
          {recent === null ? (
            <div className="tv-mobile-strips">
              {Array.from({ length: SKELETON_STRIPS }, (_, index) => (
                <SnippetStripSkeleton key={index} />
              ))}
            </div>
          ) : recent.length === 0 ? (
            <div className="tv-mobile-state">
              <div className="tv-mobile-state-figure">
                <span className="tv-mobile-state-slot" />
                <Caret height={16} />
              </div>
              <div className="tv-mobile-state-title">
                {tr('Nothing used yet', '还没有使用记录')}
              </div>
              <div className="tv-mobile-state-text">
                {tr(
                  'Snippets you reach for will appear here, most recent first.',
                  '你用过的片段会出现在这里，最近使用的排在最前。',
                )}
              </div>
            </div>
          ) : (
            <ul className="tv-mobile-strips tv-strip-list">
              {recent.map((snippet) => (
                <li key={snippet.id}>
                  <SnippetStripRow snippet={snippet} onOpen={onOpen} />
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <footer className="tv-mhome-status">
        <span className="tv-mhome-status-bar" />
        <span>{statusText}</span>
      </footer>
    </main>
  );
}
