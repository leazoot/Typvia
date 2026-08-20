import { getSnippet, historyGet, historyList, historyRestore, toIpcError } from '@typvia/shared';
import type { Snippet, VersionBody, VersionMeta } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router';
import { useEspanso } from '../../espanso/espanso-context';
import { diffLines, changedLineCount } from './line-diff';
import './history.css';

const HISTORY_PAGE_LIMIT = 100;

/** Rail timestamps read like the design's ("14:02", "Mon 09:41", "3 weeks"). */
function versionTime(createdAt: number, now: number, tr: Tr): string {
  const then = new Date(createdAt);
  const sameDay = new Date(now).toDateString() === then.toDateString();
  const time = then.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  if (sameDay) return time;
  const days = Math.floor((now - createdAt) / 86_400_000);
  if (days < 7) return `${then.toLocaleDateString(undefined, { weekday: 'short' })} ${time}`;
  if (days < 60) {
    const weeks = String(Math.floor(days / 7));
    return tr(`${weeks} weeks`, `${weeks} 周`);
  }
  return then.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/** "9 versions · 6 weeks" span label for the rail header. */
function spanLabel(entries: VersionMeta[], now: number, tr: Tr): string {
  const count = tr(
    `${String(entries.length)} ${entries.length === 1 ? 'version' : 'versions'}`,
    `${String(entries.length)} 个版本`,
  );
  const oldest = entries.at(-1);
  if (oldest === undefined || entries.length < 2) return count;
  const days = Math.max(1, Math.floor((now - oldest.createdAt) / 86_400_000));
  const span =
    days < 14
      ? tr(`${String(days)} days`, `${String(days)} 天`)
      : tr(`${String(Math.floor(days / 7))} weeks`, `${String(Math.floor(days / 7))} 周`);
  return `${count} · ${span}`;
}

type LoadState =
  | { phase: 'loading' }
  | { phase: 'locked' }
  | { phase: 'error' }
  | { phase: 'ready'; snippet: Snippet; current: number; entries: VersionMeta[] };

interface DiffState {
  compare: VersionBody;
  against: VersionBody;
}

function HistoryRail({
  entries,
  current,
  comparing,
  now,
  onPick,
}: {
  entries: VersionMeta[];
  current: number;
  comparing: number | null;
  now: number;
  onPick: (version: number) => void;
}) {
  const tr = useTr();
  return (
    <nav className="tv-hist-rail" aria-label={tr('Versions', '版本')}>
      <div className="tv-hist-rail-head">{spanLabel(entries, now, tr)}</div>
      <div className="tv-hist-rail-list">
        {entries.map((entry) => {
          const isCurrent = entry.version === current;
          const isComparing = entry.version === comparing;
          return (
            <button
              key={entry.version}
              type="button"
              className="tv-hist-row"
              data-current={isCurrent || undefined}
              data-comparing={isComparing || undefined}
              aria-pressed={isComparing}
              disabled={isCurrent}
              onClick={() => {
                onPick(entry.version);
              }}
            >
              <span className="tv-hist-row-line">
                <span className="tv-hist-row-v">v{entry.version}</span>
                {isCurrent && <span className="tv-hist-row-note">{tr('current', '当前')}</span>}
                {isComparing && (
                  <span className="tv-hist-row-note tv-hist-row-note-comparing">
                    {tr('comparing', '对比中')}
                  </span>
                )}
                <span className="tv-hist-row-time">{versionTime(entry.createdAt, now, tr)}</span>
              </span>
              <span className="tv-hist-row-title">{entry.title}</span>
            </button>
          );
        })}
      </div>
      <div className="tv-hist-rail-foot">
        <p>
          {tr(
            'Every version is local. History keeps up to 50 versions; entries older than a year thin to the newest 10.',
            '版本全部保存在本地;最多 50 个,超过一年的仅保留最近 10 个。',
          )}
        </p>
      </div>
    </nav>
  );
}

function DiffView({ diff }: { diff: DiffState }) {
  const tr = useTr();
  const rows = diffLines(diff.compare.body, diff.against.body);
  const changed = changedLineCount(rows);
  const titleChanged = diff.compare.title !== diff.against.title;
  return (
    <>
      <div className="tv-hist-diff-head">
        <div className="tv-hist-diff-title">
          <h1>
            v{diff.compare.version} → v{diff.against.version}
          </h1>
          <span className="tv-hist-diff-meta">
            {changed === 0 && !titleChanged
              ? tr('no line changes', '没有行变更')
              : tr(
                  `${String(changed)} ${changed === 1 ? 'line' : 'lines'} changed${titleChanged ? ' · title changed' : ''}`,
                  `${String(changed)} 行有变更${titleChanged ? ' · 标题有变更' : ''}`,
                )}
          </span>
        </div>
        <p>
          {tr(
            `Comparing v${String(diff.compare.version)} with the current version — pick any version on the left to compare`,
            `对比 v${String(diff.compare.version)} 与当前版本 — 选择左侧任一版本即可比较`,
          )}
        </p>
      </div>
      <div className="tv-hist-diff-scroll">
        <div className="tv-hist-label">{tr('Body', '正文')}</div>
        <div className="tv-hist-diff" role="table" aria-label={tr('Line changes', '行变更')}>
          {rows.map((row, index) => (
            <div key={index} className="tv-hist-diff-row" data-kind={row.kind} role="row">
              <span className="tv-hist-diff-gutter" role="cell">
                {row.line}
              </span>
              <span className="tv-hist-diff-text" role="cell">
                {row.kind !== 'same' && (
                  <span
                    className="tv-hist-diff-sign"
                    aria-label={
                      row.kind === 'removed' ? tr('removed', '已删除') : tr('added', '已新增')
                    }
                  >
                    {row.kind === 'removed' ? '−' : '+'}
                  </span>
                )}
                {row.text === '' ? ' ' : row.text}
              </span>
            </div>
          ))}
        </div>
        <div className="tv-hist-label">{tr('Everything else', '其他字段')}</div>
        <div className="tv-hist-fields">
          <span className="tv-hist-fields-head">{tr('field', '字段')}</span>
          <span className="tv-hist-fields-head">v{diff.compare.version}</span>
          <span className="tv-hist-fields-head">
            {tr(
              `v${String(diff.against.version)} · current`,
              `v${String(diff.against.version)} · 当前`,
            )}
          </span>
          <span className="tv-hist-fields-name">{tr('Title', '标题')}</span>
          <span className="tv-hist-fields-old">{diff.compare.title}</span>
          <span className={titleChanged ? 'tv-hist-fields-new' : 'tv-hist-fields-same'}>
            {titleChanged ? diff.against.title : tr('unchanged', '未更改')}
          </span>
        </div>
      </div>
    </>
  );
}

/**
 * Version history: version rail → diff → field table
 * → action bar. Restore writes a new version forward — no confirm dialog,
 * the footer states the consequence instead. No motion: diffs are read.
 */
export function HistoryPage() {
  const tr = useTr();
  const { id } = useParams();
  const navigate = useNavigate();
  const { notifyMutation } = useEspanso();
  const [state, setState] = useState<LoadState>({ phase: 'loading' });
  const [diff, setDiff] = useState<DiffState | null>(null);
  const [restoredTo, setRestoredTo] = useState<number | null>(null);
  const [now] = useState(() => Date.now());

  const load = useCallback(
    async (compare: number | null) => {
      if (id === undefined) return;
      try {
        const [snippet, history] = await Promise.all([
          getSnippet(id),
          historyList(id, HISTORY_PAGE_LIMIT, 0),
        ]);
        setState({ phase: 'ready', snippet, current: history.current, entries: history.entries });
        // Default comparison: the newest non-current version, as in the design.
        const target =
          compare ?? history.entries.find((e) => e.version !== history.current)?.version;
        if (target !== undefined && history.entries.some((e) => e.version === history.current)) {
          const [old, live] = await Promise.all([
            historyGet(id, target),
            historyGet(id, history.current),
          ]);
          setDiff({ compare: old, against: live });
        } else {
          setDiff(null);
        }
      } catch (raw) {
        setState(
          toIpcError(raw).code === 'permission_denied' ? { phase: 'locked' } : { phase: 'error' },
        );
      }
    },
    [id],
  );

  useEffect(() => {
    void load(null);
  }, [load]);

  const restore = async () => {
    if (id === undefined || diff === null) return;
    try {
      const updated = await historyRestore(id, diff.compare.version);
      setRestoredTo(updated.version);
      // The body changed under Espanso's feet; trigger a config re-sync.
      notifyMutation();
      await load(null);
    } catch {
      setState({ phase: 'error' });
    }
  };

  const backToEditing = () => {
    void navigate(id === undefined ? '/library' : `/editor/${id}`);
  };

  return (
    <main className="tv-hist">
      <div className="tv-ed-top">
        <nav aria-label={tr('Breadcrumb', '面包屑导航')} className="tv-ed-crumbs">
          <button
            type="button"
            className="tv-ed-crumb-link"
            onClick={() => void navigate('/library')}
          >
            {tr('Library', '片段库')}
          </button>
          <span aria-hidden="true" className="tv-ed-crumb-sep">
            /
          </span>
          {state.phase === 'ready' && (
            <>
              <button type="button" className="tv-ed-crumb-link" onClick={backToEditing}>
                {state.snippet.title === ''
                  ? tr('Untitled snippet', '未命名片段')
                  : state.snippet.title}
              </button>
              <span aria-hidden="true" className="tv-ed-crumb-sep">
                /
              </span>
            </>
          )}
          <span className="tv-ed-crumb tv-ed-crumb-current">{tr('History', '历史')}</span>
        </nav>
        <button type="button" className="tv-hist-back" onClick={backToEditing}>
          {tr('Back to editing', '返回编辑')}
        </button>
      </div>

      {state.phase === 'loading' && (
        <div className="tv-hist-wait" aria-busy="true">
          <span className="tv-hist-wait-row" />
          <span className="tv-hist-wait-row" />
          <span className="tv-hist-wait-row" />
        </div>
      )}

      {state.phase === 'locked' && (
        <div className="tv-hist-notice">
          <span className="tv-hist-notice-mark" aria-hidden="true" />
          <p>
            {tr(
              "This snippet's history is intact — unlock the Vault to read it.",
              '历史记录完好;解锁保险库后即可查看。',
            )}
          </p>
        </div>
      )}

      {state.phase === 'error' && (
        <div className="tv-hist-notice">
          <p>
            {tr(
              "Your snippet is intact — its history just isn't available right now.",
              '片段本身完好,只是历史暂时不可用。',
            )}
          </p>
        </div>
      )}

      {state.phase === 'ready' && (
        <div className="tv-hist-columns">
          <HistoryRail
            entries={state.entries}
            current={state.current}
            comparing={diff?.compare.version ?? null}
            now={now}
            onPick={(version) => {
              setRestoredTo(null);
              void load(version);
            }}
          />
          <section className="tv-hist-main">
            {diff === null ? (
              <div className="tv-hist-notice">
                <p>
                  {tr(
                    'Only one version so far — every later save will appear here.',
                    '目前只有一个版本;之后的每次保存都会记录在这里。',
                  )}
                </p>
              </div>
            ) : (
              <DiffView diff={diff} />
            )}
            <div className="tv-hist-actions">
              {diff !== null && (
                <button type="button" className="tv-hist-restore" onClick={() => void restore()}>
                  {tr(
                    `Restore v${String(diff.compare.version)}`,
                    `恢复 v${String(diff.compare.version)}`,
                  )}
                </button>
              )}
              <span className="tv-hist-actions-note" role="status">
                {restoredTo !== null
                  ? tr(
                      `Restored as v${String(restoredTo)} — nothing was lost.`,
                      `已恢复为 v${String(restoredTo)}——没有任何内容丢失。`,
                    )
                  : diff !== null
                    ? tr(
                        `Restoring writes a new v${String(state.current + 1)} — v${String(state.current)} is never lost.`,
                        `恢复会写入新的 v${String(state.current + 1)}——v${String(state.current)} 不会丢失。`,
                      )
                    : ''}
              </span>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}
