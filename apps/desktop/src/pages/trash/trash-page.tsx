// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { deleteSnippetForever, listTrash, purgeExpiredTrash, restoreSnippet } from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { TypeMark, markForType, useTr } from '@typvia/ui';
import { useCallback, useEffect, useState } from 'react';
import { useEspanso } from '../../espanso/espanso-context';
import { Action, OverflowMenu, Row, useContextMenu } from '../../workspace/kit';
import './trash.css';

const PAGE_LIMIT = 500;
const RETENTION_DAYS = 30;
const DAY_MS = 24 * 60 * 60 * 1000;
/** How long the "Restored" word stays where the row was. */
const RESTORED_NOTE_MS = 1200;

/** Whole days before a trashed row is purged (never below zero). */
function daysLeft(deletedAt: number, now: number): number {
  const gone = deletedAt + RETENTION_DAYS * DAY_MS;
  return Math.max(0, Math.ceil((gone - now) / DAY_MS));
}

/** "3 days ago" in whole days — deletion time never needs a clock. */
function deletedWhen(deletedAt: number, now: number, tr: ReturnType<typeof useTr>): string {
  const days = Math.floor((now - deletedAt) / DAY_MS);
  if (days <= 0) return tr('Deleted today', '今天删除');
  if (days === 1) return tr('Deleted yesterday', '昨天删除');
  return tr(`Deleted ${String(days)} days ago`, `${String(days)} 天前删除`);
}

/**
 * Trash — a quiet recovery space, not a page that deserves
 * the whole canvas. Restore is the visible action; deleting forever hides in
 * ··· and always asks with the real number, in place.
 */
export function TrashPage() {
  const tr = useTr();
  // Restores put triggers back into the espanso config.
  const { notifyMutation } = useEspanso();
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [leaving, setLeaving] = useState<ReadonlySet<string>>(new Set());
  const [restored, setRestored] = useState<ReadonlySet<string>>(new Set());
  const [confirmEmpty, setConfirmEmpty] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [confirmSelection, setConfirmSelection] = useState(false);
  const context = useContextMenu(tr('Trash actions', '回收站操作'));

  const reload = useCallback(() => {
    listTrash(PAGE_LIMIT, 0)
      .then((trashed) => {
        setRows(trashed);
        setFailed(false);
      })
      .catch(() => {
        setFailed(true);
      });
  }, []);

  useEffect(() => {
    // The 30-day policy is enforced on entry; expired rows never render.
    purgeExpiredTrash()
      .catch(() => {
        // Purge failing must not block the listing.
      })
      .finally(reload);
  }, [reload]);

  const now = Date.now();

  /** Shrinks the row away first, then reconciles with the host. */
  const leave = (id: string, run: Promise<unknown>, note: boolean) => {
    setLeaving((current) => new Set(current).add(id));
    void run
      .then(() => {
        if (note) {
          setRestored((current) => new Set(current).add(id));
          setTimeout(() => {
            setRestored((current) => {
              const next = new Set(current);
              next.delete(id);
              return next;
            });
            reload();
          }, RESTORED_NOTE_MS);
        } else {
          reload();
        }
      })
      .catch(() => {
        setLeaving((current) => {
          const next = new Set(current);
          next.delete(id);
          return next;
        });
        reload();
      });
  };

  const restore = (snippet: Snippet) =>
    leave(snippet.id, restoreSnippet(snippet.id).then(notifyMutation), true);

  const purge = (snippet: Snippet) => leave(snippet.id, deleteSnippetForever(snippet.id), false);

  const toggleSelected = (id: string) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const runOnSelection = (each: (id: string) => Promise<unknown>) => {
    const ids = [...selected];
    setSelected(new Set());
    setConfirmSelection(false);
    setLeaving((current) => new Set([...current, ...ids]));
    void Promise.allSettled(ids.map(each)).then(() => {
      void notifyMutation();
      reload();
    });
  };

  const emptyTrash = () => {
    const ids = (rows ?? []).map((row) => row.id);
    setConfirmEmpty(false);
    void Promise.allSettled(ids.map((id) => deleteSnippetForever(id))).then(reload);
  };

  const count = rows?.length ?? 0;

  return (
    <div className="tvt">
      <div className="tvt-head">
        <div>
          <h1>{tr('Trash', '回收站')}</h1>
          <div className="tvt-sub">
            {rows === null
              ? tr(`Kept for ${String(RETENTION_DAYS)} days`, `保留 ${String(RETENTION_DAYS)} 天`)
              : tr(
                  `${String(count)} item${count === 1 ? '' : 's'} · kept for ${String(RETENTION_DAYS)} days`,
                  `${String(count)} 项 · 保留 ${String(RETENTION_DAYS)} 天`,
                )}
          </div>
        </div>
        {count > 0 && (
          <OverflowMenu
            label={tr('Trash options', '回收站选项')}
            items={[
              {
                label: tr('Empty Trash', '清空回收站'),
                danger: true,
                onSelect: () => setConfirmEmpty(true),
              },
            ]}
          />
        )}
      </div>

      <div className="tvt-list">
        {failed ? (
          <div className="tvt-empty">
            <strong>{tr('Your library is unaffected.', '你的片段库不受影响。')}</strong>
            {tr('The trash list failed to load just now.', '只是回收站列表刚才未能加载。')}
            <div style={{ marginTop: 12 }}>
              <button type="button" className="tvw-chip" onClick={reload}>
                {tr('Retry', '重试')}
              </button>
            </div>
          </div>
        ) : rows !== null && rows.length === 0 ? (
          <div className="tvt-empty">
            <strong>{tr('The trash is clean.', '回收站很干净。')}</strong>
            {tr(
              `Deleted snippets wait here for ${String(RETENTION_DAYS)} days before they go for good.`,
              `已删除的片段会在这里保留 ${String(RETENTION_DAYS)} 天,之后才会永久删除。`,
            )}
            <div aria-hidden="true" className="tvt-empty-rule" />
          </div>
        ) : (
          rows?.map((snippet) => {
            if (restored.has(snippet.id)) {
              return (
                <p key={snippet.id} className="tvt-restored" role="status">
                  {snippet.securityLevel === 'sensitive'
                    ? tr('Restored to the Vault', '已恢复到保险库')
                    : tr('Restored to the Library', '已恢复到片段库')}
                </p>
              );
            }
            const menu = [
              {
                label: tr('Delete permanently', '永久删除'),
                danger: true,
                onSelect: () => purge(snippet),
              },
            ];
            return (
              <Row
                key={snippet.id}
                className={`tvt-row ${leaving.has(snippet.id) ? 'is-leaving' : ''} ${
                  selected.has(snippet.id) ? 'is-selected' : ''
                }`}
                label={snippet.title}
                onOpen={() => toggleSelected(snippet.id)}
                onContextMenu={(event) => context.open(event, menu)}
              >
                <TypeMark code={markForType(snippet.snippetType)} />
                <span className="tvw-grow">
                  <span className="tvw-row-title">{snippet.title}</span>
                  <span className="tvt-when">
                    {snippet.deletedAt === null ? '' : deletedWhen(snippet.deletedAt, now, tr)}
                  </span>
                </span>
                <span className="tvt-tail">
                  {snippet.deletedAt !== null && (
                    <span className="tvt-life">
                      <span aria-hidden="true" className="tvw-thread">
                        <span
                          style={{
                            width: `${String(Math.round((daysLeft(snippet.deletedAt, now) / RETENTION_DAYS) * 100))}%`,
                          }}
                        />
                      </span>
                      {tr(
                        `${String(daysLeft(snippet.deletedAt, now))} days`,
                        `剩 ${String(daysLeft(snippet.deletedAt, now))} 天`,
                      )}
                    </span>
                  )}
                  <span className="tvw-hover-actions tvt-actions">
                    <Action label={tr('Restore', '恢复')} cta onRun={() => restore(snippet)} />
                    <OverflowMenu label={tr('More actions', '更多操作')} items={menu} />
                  </span>
                </span>
              </Row>
            );
          })
        )}
      </div>

      {selected.size > 0 && (
        <div className="tvw-selbar" role="group" aria-label={tr('Selection', '已选片段')}>
          {confirmSelection ? (
            <>
              <span className="tvw-selbar-count">
                {tr(
                  `Delete ${String(selected.size)} item${selected.size === 1 ? '' : 's'} forever?`,
                  `永久删除 ${String(selected.size)} 项?`,
                )}
              </span>
              <button
                type="button"
                className="tvt-danger"
                onClick={() => runOnSelection(deleteSnippetForever)}
              >
                {tr(
                  `Delete ${String(selected.size)} item${selected.size === 1 ? '' : 's'}`,
                  `删除 ${String(selected.size)} 项`,
                )}
              </button>
              <Action label={tr('Cancel', '取消')} onRun={() => setConfirmSelection(false)} />
            </>
          ) : (
            <>
              <span className="tvw-selbar-count">
                {tr(`${String(selected.size)} selected`, `已选 ${String(selected.size)} 项`)}
              </span>
              <Action
                label={tr(`Restore ${String(selected.size)}`, `恢复 ${String(selected.size)} 项`)}
                cta
                onRun={() => runOnSelection(restoreSnippet)}
              />
              <Action
                label={tr('Delete forever', '永久删除')}
                onRun={() => setConfirmSelection(true)}
              />
              <Action label={tr('Done', '完成')} onRun={() => setSelected(new Set())} />
            </>
          )}
        </div>
      )}

      {confirmEmpty && (
        <>
          <div
            aria-hidden="true"
            className="tvw-scrim-layer"
            onClick={() => setConfirmEmpty(false)}
          />
          <div className="tvt-confirm" role="dialog" aria-label={tr('Empty Trash', '清空回收站')}>
            <div className="tvt-confirm-title">
              {tr(
                `Delete ${String(count)} item${count === 1 ? '' : 's'} in the trash forever?`,
                `永久删除回收站中的 ${String(count)} 项?`,
              )}
            </div>
            <div className="tvt-confirm-note">
              {tr('This cannot be undone.', '此操作无法撤销。')}
            </div>
            <div className="tvt-confirm-actions">
              <Action label={tr('Cancel', '取消')} onRun={() => setConfirmEmpty(false)} />
              <button type="button" className="tvt-danger" onClick={emptyTrash}>
                {tr('Delete forever', '永久删除')}
              </button>
            </div>
          </div>
        </>
      )}
      {context.node}
    </div>
  );
}
