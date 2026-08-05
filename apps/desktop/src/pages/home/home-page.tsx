import { libraryCounts, listSnippetPage } from '@typvia/shared';
import type { LibraryCounts, Snippet } from '@typvia/shared';
import { Caret, SearchLine, TypeMark } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { markFor } from '../library/preview';
import './home.css';

const LEDGER_LIMIT = 50;

/** Day bucket for the chronological ledger (never a ranking). */
function dayLabel(usedAt: number, now: number): string {
  const day = 24 * 60 * 60 * 1000;
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);
  if (usedAt >= startOfToday) return 'Reached for today · 今天用过';
  if (usedAt >= startOfToday - day) return 'Yesterday · 昨天';
  return 'Earlier · 更早';
}

function clockLabel(usedAt: number): string {
  const at = new Date(usedAt);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

interface LedgerGroup {
  day: string;
  clock: string;
  rows: Snippet[];
}

/** Groups used snippets by day, then by minute-of-use, preserving order. */
function groupLedger(rows: Snippet[], now: number): LedgerGroup[] {
  const groups: LedgerGroup[] = [];
  for (const row of rows) {
    if (row.lastUsedAt === null) continue;
    const day = dayLabel(row.lastUsedAt, now);
    const clock = clockLabel(row.lastUsedAt);
    const last = groups[groups.length - 1];
    if (last !== undefined && last.day === day && last.clock === clock) {
      last.rows.push(row);
    } else {
      groups.push({ day, clock, rows: [row] });
    }
  }
  return groups;
}

/**
 * Home (design Phase 2 · 1a): a log, not a dashboard. The 40px search line
 * is the only large type; below it a chronological usage ledger with time
 * separators (never ranked), and a 300px status rail. BATCH-01 shows local
 * state only — channels that need later batches read "not configured".
 */
export function HomePage() {
  const navigate = useNavigate();
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [used, setUsed] = useState<Snippet[] | null>(null);

  useEffect(() => {
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

  const groups = groupLedger(used ?? [], Date.now());
  const total = counts?.total ?? 0;
  const firstRun = counts !== null && total === 0;
  let ledgerRow = -1;

  return (
    <main className="tv-home">
      <div className="tv-home-hero">
        <div className="tv-home-hero-top">
          <span className="tv-home-label">Start typing · 直接开始输入</span>
          {counts !== null && (
            <span className="tv-home-meta">
              {total.toLocaleString('en-US')} snippets · all local
            </span>
          )}
        </div>
        <div className="tv-home-search">
          <SearchLine
            scale="hero"
            value=""
            onChange={(value) => {
              if (value !== '') void navigate('/library', { state: { query: value } });
            }}
            placeholder="Search your snippets"
            label="Search snippets"
            trailing={
              <span className="tv-home-hero-actions">
                <button
                  type="button"
                  className="tv-home-new"
                  onClick={() => void navigate('/editor')}
                >
                  New
                </button>
              </span>
            }
          />
        </div>
      </div>

      <div className="tv-home-columns">
        <section className="tv-home-ledger" aria-label="Usage ledger">
          {firstRun ? (
            <div className="tv-home-state">
              <div className="tv-home-state-figure">
                <span className="tv-home-state-slot" />
                <Caret height={16} />
              </div>
              <div className="tv-home-state-title">Save your first snippet</div>
              <div className="tv-home-state-text">
                Anything you type twice belongs here — commands, replies, prompts.
              </div>
              <button
                type="button"
                className="tv-home-state-primary"
                onClick={() => void navigate('/editor')}
              >
                New snippet
              </button>
            </div>
          ) : groups.length === 0 ? (
            used !== null && (
              <div className="tv-home-state">
                <div className="tv-home-state-figure">
                  <span className="tv-home-state-slot" />
                  <Caret height={16} />
                </div>
                <div className="tv-home-state-title">Nothing used yet</div>
                <div className="tv-home-state-text">
                  Snippets you insert will appear here in the order you reached for them.
                </div>
                <button
                  type="button"
                  className="tv-home-state-primary"
                  onClick={() => void navigate('/library')}
                >
                  Open Library
                </button>
              </div>
            )
          ) : (
            groups.map((group, groupIndex) => {
              const previous = groups[groupIndex - 1];
              const newDay = previous === undefined || previous.day !== group.day;
              return (
                <div key={`${group.day}-${group.clock}-${String(groupIndex)}`}>
                  {newDay && (
                    <div className="tv-home-day">
                      <span className="tv-home-label">{group.day}</span>
                      <span className="tv-home-day-rule" />
                    </div>
                  )}
                  <div className="tv-home-clock">
                    <span>{group.clock}</span>
                    <span className="tv-home-clock-rule" />
                  </div>
                  {group.rows.map((snippet) => {
                    ledgerRow += 1;
                    return (
                      <button
                        key={snippet.id}
                        type="button"
                        className="tv-home-row"
                        style={{ animationDelay: `${String(ledgerRow * 40)}ms` }}
                        onClick={() => void navigate(`/editor/${snippet.id}`)}
                      >
                        <TypeMark code={markFor(snippet.snippetType)} />
                        <span className="tv-home-row-title">{snippet.title}</span>
                        <span className="tv-home-row-cn" lang="zh-Hans">
                          {snippet.description ?? ''}
                        </span>
                        <span aria-hidden="true" className="tv-home-row-preview">
                          {snippet.body ?? ''}
                        </span>
                        {snippet.trigger !== null && (
                          <span className="tv-home-row-trigger">{snippet.trigger}</span>
                        )}
                        <span className="tv-home-row-uses">{`${String(snippet.usageCount)}×`}</span>
                      </button>
                    );
                  })}
                </div>
              );
            })
          )}
        </section>

        <aside className="tv-home-rail" aria-label="Status">
          <div className="tv-home-label tv-home-rail-label">Where it can arrive · 可以抵达</div>
          <div aria-hidden="true" className="tv-home-route">
            <span className="tv-home-route-node tv-home-route-node-here" />
            <span className="tv-home-route-line" />
            <span className="tv-home-route-node tv-home-route-node-later" />
            <span className="tv-home-route-line tv-home-route-line-faint" />
            <span className="tv-home-route-node tv-home-route-node-dashed" />
          </div>
          <div className="tv-home-rail-rows">
            <div className="tv-home-rail-row">
              <span className="tv-home-rail-strong">This device</span>
              <span className="tv-home-rail-meta">everything local</span>
            </div>
            <div className="tv-home-rail-row">
              <span>Pair a device</span>
              <span className="tv-home-rail-meta">arrives with sync</span>
            </div>
          </div>

          <div className="tv-home-rail-divider" />
          <div className="tv-home-label tv-home-rail-label">Library · 本地状态</div>
          <div className="tv-home-rail-rows">
            <div className="tv-home-rail-row">
              <span>Snippets</span>
              <span className="tv-home-rail-meta">
                {counts === null ? '—' : counts.total.toLocaleString('en-US')}
              </span>
            </div>
            <div className="tv-home-rail-row">
              <span>Trash</span>
              <span className="tv-home-rail-meta">
                {counts === null ? '—' : counts.trash.toLocaleString('en-US')}
              </span>
            </div>
          </div>

          <div className="tv-home-rail-divider" />
          <div className="tv-home-label tv-home-rail-label">Ready to type · 输入通道</div>
          <div className="tv-home-channels">
            {['Global panel', 'Espanso', 'Sync', 'iOS keyboard', 'Android IME', 'Vault'].map(
              (channel) => (
                <div key={channel} className="tv-home-channel">
                  <span aria-hidden="true" className="tv-home-channel-dot" />
                  <span>{channel}</span>
                  <span className="tv-home-rail-meta">not configured</span>
                </div>
              ),
            )}
          </div>

          <div className="tv-home-rail-foot">
            Nothing leaves this Mac unless you ask.
            <br />
            <span lang="zh-Hans">未经允许,内容不会离开本机。</span>
          </div>
        </aside>
      </div>
    </main>
  );
}
