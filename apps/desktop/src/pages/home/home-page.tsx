import {
  copySnippet,
  libraryCounts,
  listSnippetPage,
  searchLibrary,
  trashSnippet,
} from '@typvia/shared';
import type { LibraryCounts, Snippet } from '@typvia/shared';
import { TypeMark, markForType, useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';
import {
  Action,
  CopyAction,
  Dot,
  Glyph,
  Mark,
  OverflowMenu,
  Row,
  TriggerToken,
  useContextMenu,
  type MenuEntry,
} from '../../workspace/kit';
import './home.css';

const LEDGER_LIMIT = 40;
const RESULT_LIMIT = 8;
const PLACEHOLDER_ROTATION_MS = 6000;

type DayBucket = 'today' | 'yesterday' | 'earlier';

/** Day bucket for the chronological ledger (never a ranking). */
function dayBucket(usedAt: number, now: number): DayBucket {
  const day = 24 * 60 * 60 * 1000;
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);
  if (usedAt >= startOfToday) return 'today';
  if (usedAt >= startOfToday - day) return 'yesterday';
  return 'earlier';
}

function dayHeading(bucket: DayBucket, tr: Tr): string {
  switch (bucket) {
    case 'today':
      return tr('Today', '今天');
    case 'yesterday':
      return tr('Yesterday', '昨天');
    case 'earlier':
      return tr('Earlier', '更早');
  }
}

function clockLabel(usedAt: number): string {
  const at = new Date(usedAt);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

interface LedgerRow {
  snippet: Snippet;
  day: DayBucket;
  /** Set on the first row of a minute; the rest keep the column empty. */
  clock: string | null;
}

/** Rows in use order, stamped with the day and minute separators they open. */
function ledgerRows(rows: readonly Snippet[], now: number): LedgerRow[] {
  const out: LedgerRow[] = [];
  let lastClock = '';
  for (const snippet of rows) {
    if (snippet.lastUsedAt === null) continue;
    const clock = clockLabel(snippet.lastUsedAt);
    out.push({
      snippet,
      day: dayBucket(snippet.lastUsedAt, now),
      clock: clock === lastClock ? null : clock,
    });
    lastClock = clock;
  }
  return out;
}

/**
 * Home — the launch surface. One search line is the whole
 * page: typing anywhere lands in it, results replace the ledger as they
 * arrive, and what search cannot find becomes a command instead.
 */
export function HomePage() {
  const tr = useTr();
  const navigate = useNavigate();
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [used, setUsed] = useState<Snippet[] | null>(null);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Snippet[]>([]);
  const [focused, setFocused] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [placeholder, setPlaceholder] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const createWrap = useRef<HTMLSpanElement>(null);
  const context = useContextMenu(tr('Snippet actions', '片段操作'));

  const reload = useCallback(() => {
    libraryCounts()
      .then(setCounts)
      .catch(() => {
        setCounts(null);
      });
    listSnippetPage('recent', null, null, LEDGER_LIMIT, 0)
      .then(setUsed)
      .catch(() => {
        setUsed([]);
      });
  }, []);
  useEffect(reload, [reload]);

  // Every keystroke replaces the results — no debounce, no transition.
  useEffect(() => {
    const needle = query.trim();
    if (needle === '') {
      setResults([]);
      return;
    }
    let current = true;
    searchLibrary(needle, RESULT_LIMIT)
      .then((rows) => {
        if (current) setResults(rows);
      })
      .catch(() => {
        if (current) setResults([]);
      });
    return () => {
      current = false;
    };
  }, [query]);

  // The placeholder drifts between the things one can search for — slowly,
  // and never while the reader asked for less motion.
  useEffect(() => {
    if (
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches
    ) {
      return;
    }
    const timer = setInterval(() => setPlaceholder((index) => index + 1), PLACEHOLDER_ROTATION_MS);
    return () => clearInterval(timer);
  }, []);

  // "Start typing" taken literally: a printable key anywhere on the
  // page lands in the search line, and "/" just focuses it.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable);
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key === 'Escape' && !typing) {
        setQuery('');
        return;
      }
      if (typing) return;
      if (event.key === '/') {
        event.preventDefault();
        input.current?.focus();
        return;
      }
      if (event.key.length === 1 && event.key !== ' ') {
        event.preventDefault();
        setQuery((current) => current + event.key);
        input.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    if (!createOpen) return;
    const away = (event: MouseEvent) => {
      if (createWrap.current?.contains(event.target as Node) !== true) setCreateOpen(false);
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [createOpen]);

  const placeholders = [
    tr('Search titles…', '搜索标题…'),
    tr('Search :email…', '搜索 :email…'),
    tr('Search tags…', '搜索标签…'),
    tr('Search contents…', '搜索内容…'),
  ];
  const total = counts?.total ?? 0;
  const searching = query.trim() !== '';
  const rows = ledgerRows(used ?? [], Date.now());
  const firstRun = counts !== null && total === 0;

  const openSnippet = (snippet: Snippet) =>
    void navigate(snippet.securityLevel === 'sensitive' ? '/vault' : `/editor/${snippet.id}`);

  const menuFor = (snippet: Snippet): MenuEntry[] => [
    {
      label: tr('Open in Library', '在片段库中打开'),
      onSelect: () => void navigate('/library', { state: { query: snippet.title } }),
    },
    'divider',
    {
      label: tr('Move to Trash', '移到回收站'),
      danger: true,
      onSelect: () => {
        void trashSnippet(snippet.id).then(() => {
          setUsed((current) =>
            current === null ? current : current.filter((row) => row.id !== snippet.id),
          );
          reload();
        });
      },
    },
  ];

  const renderRow = (snippet: Snippet, clock: string | null) => {
    const sensitive = snippet.securityLevel === 'sensitive';
    return (
      <Row
        key={snippet.id}
        className="tvh-row"
        label={snippet.title}
        onOpen={() => openSnippet(snippet)}
        onContextMenu={(event) => {
          if (!sensitive) context.open(event, menuFor(snippet));
        }}
      >
        <span aria-hidden="true" className="tvh-row-clock">
          {clock ?? ''}
        </span>
        <TypeMark code={markForType(snippet.snippetType)} />
        <span className="tvw-grow">
          <span className="tvw-row-title">{snippet.title}</span>
          <span aria-hidden="true" className="tvw-row-preview">
            {sensitive ? tr('Secret', '密文') : (snippet.body ?? '')}
          </span>
        </span>
        <span className="tvh-row-tail">
          {sensitive ? (
            <>
              {snippet.trigger !== null && <TriggerToken trigger={snippet.trigger} />}
              <span className="tvh-locked">{tr('in Vault', '在保险库')}</span>
            </>
          ) : (
            <>
              <span className="tvw-row-rest">
                {snippet.trigger !== null && <TriggerToken trigger={snippet.trigger} />}
                <span className="tvh-row-uses">{`${String(snippet.usageCount)}×`}</span>
              </span>
              <span className="tvw-hover-actions tvh-row-actions">
                <CopyAction
                  label={tr('Copy', '复制')}
                  doneLabel={tr('Copied', '已复制')}
                  onCopy={() => copySnippet(snippet.id)}
                />
                <Action
                  label={tr('Edit', '编辑')}
                  onRun={() => void navigate(`/editor/${snippet.id}`)}
                />
                <OverflowMenu label={tr('More actions', '更多操作')} items={menuFor(snippet)} />
              </span>
            </>
          )}
        </span>
      </Row>
    );
  };

  return (
    <div className="tvh" data-focus={focused ? 'true' : 'false'}>
      <p className="tvh-prompt tvh-quiet">{tr('What are you looking for?', '找点什么?')}</p>

      <div className="tvh-search">
        <Glyph name="search" />
        <input
          ref={input}
          type="text"
          value={query}
          aria-label={tr('Search snippets', '搜索片段')}
          placeholder={placeholders[placeholder % placeholders.length]}
          onChange={(event) => setQuery(event.target.value)}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
        />
        <span ref={createWrap} className="tvh-create-wrap">
          <button
            type="button"
            className="tvh-plus"
            aria-label={tr('New', '新建')}
            aria-expanded={createOpen}
            onClick={() => setCreateOpen((open) => !open)}
          >
            ＋
          </button>
          {createOpen && (
            <span className="tvw-pop tvh-create" role="menu" aria-label={tr('New', '新建')}>
              <span className="tvw-pop-label">{tr('New', '新建')}</span>
              {(
                [
                  {
                    mark: 'TX',
                    label: tr('Snippet', '普通片段'),
                    desc: tr('Text, command or template', '文本、命令或模板'),
                    to: '/editor',
                    state: undefined,
                  },
                  {
                    mark: 'SC',
                    label: tr('Secret', '敏感片段'),
                    desc: tr('Encrypted on this Mac', '本机加密保存'),
                    to: '/vault',
                    state: { create: true },
                  },
                  {
                    mark: 'AI',
                    label: tr('AI action', 'AI 动作'),
                    desc: tr('Run AI over selected text', '对选中文本执行 AI'),
                    to: '/ai',
                    state: { create: true },
                  },
                ] as const
              ).map((entry) => (
                <button
                  key={entry.mark}
                  type="button"
                  role="menuitem"
                  className="tvw-pop-item"
                  onClick={() => {
                    setCreateOpen(false);
                    void navigate(entry.to, entry.state ? { state: entry.state } : undefined);
                  }}
                >
                  <Mark code={entry.mark} label={entry.label} />
                  <span>
                    <span className="tvw-pop-name">{entry.label}</span>
                    <span className="tvw-pop-desc">{entry.desc}</span>
                  </span>
                  <span />
                </button>
              ))}
            </span>
          )}
        </span>
      </div>

      <div className="tvh-meta tvh-quiet">
        <span>
          {counts === null
            ? '—'
            : tr(
                `${total.toLocaleString('en-US')} snippet${total === 1 ? '' : 's'}`,
                `${total.toLocaleString('en-US')} 个片段`,
              )}
        </span>
        <span className="tvh-meta-here">
          <Dot kind="ok" />
          {tr('This Mac', '本机')}
        </span>
      </div>

      {searching ? (
        <>
          <section className="tvh-section" aria-label={tr('Matches', '匹配')}>
            <div className="tvh-section-head">
              <span className="tvw-label">{tr('Matches', '匹配')}</span>
            </div>
            {results.length === 0 ? (
              <p className="tvh-nothing">
                {tr('No snippet matches that yet.', '还没有匹配的片段。')}
              </p>
            ) : (
              results.map((snippet) => renderRow(snippet, null))
            )}
          </section>
          <section className="tvh-section" aria-label={tr('Commands', '命令')}>
            <div className="tvh-section-head">
              <span className="tvw-label">{tr('Commands', '命令')}</span>
            </div>
            <Row
              className="tvh-command"
              label={tr('Create a snippet', '创建片段')}
              onOpen={() =>
                void navigate('/editor', {
                  state: query.trim().startsWith(':')
                    ? { draftTrigger: query.trim() }
                    : { draftTitle: query.trim() },
                })
              }
            >
              <span aria-hidden="true" className="tvh-command-plus">
                ＋
              </span>
              <span className="tvh-command-text">
                {query.trim().startsWith(':')
                  ? tr(`Create the trigger “${query.trim()}”`, `创建触发词「${query.trim()}」`)
                  : tr(`Create a snippet “${query.trim()}”`, `新建片段「${query.trim()}」`)}
              </span>
            </Row>
          </section>
        </>
      ) : firstRun ? (
        <div className="tvh-empty tvh-quiet">
          <p className="tvh-empty-line">
            {tr('It is quiet here.', '这里还很安静。')}
            <span aria-hidden="true" className="tvw-key">
              |
            </span>
          </p>
          <p className="tvw-empty" style={{ padding: '8px 0 0' }}>
            {tr(
              'Type your first piece of text and Typvia will remember it for you.',
              '输入你的第一段文字,Typvia 会替你记住它。',
            )}
          </p>
          <button type="button" className="tvw-chip" onClick={() => void navigate('/editor')}>
            {tr('Create the first snippet', '创建第一个片段')}
          </button>
        </div>
      ) : (
        <section className="tvh-section tvh-quiet" aria-label={tr('Recent', '最近使用')}>
          <div className="tvh-section-head">
            <span className="tvw-label">{tr('Recent', '最近使用')}</span>
          </div>
          {rows.length === 0
            ? used !== null && (
                <p className="tvh-nothing">
                  {tr(
                    'Snippets you insert will appear here, in the order you reached for them.',
                    '插入过的片段会按使用的先后顺序显示在这里。',
                  )}
                </p>
              )
            : rows.map((row, index) => {
                const previous = rows[index - 1];
                const newDay = previous === undefined || previous.day !== row.day;
                return (
                  <div key={row.snippet.id}>
                    {newDay && <div className="tvh-day">{dayHeading(row.day, tr)}</div>}
                    {renderRow(row.snippet, row.clock)}
                  </div>
                );
              })}
        </section>
      )}
      {context.node}
    </div>
  );
}
