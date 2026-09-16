// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { deleteSnippetForever, listTrash, purgeExpiredTrash, restoreSnippet } from '@typvia/shared';
import type { Folder, Snippet } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { useCallback, useEffect, useState, type KeyboardEvent } from 'react';
import { useEspanso } from '../../espanso/espanso-context';
import { Mascot } from '../../paper/mascot';
import { usePaperMenu, type MenuEntry, type MenuPoint } from '../../paper/menu';
import { useCommand } from '../../workspace/commands';
import { announceLibraryChange, useLibraryChange } from '../../workspace/library-events';
import { useUndo } from '../../workspace/undo';
import { fetchFolders } from '../library/library-data';
import { firstLine } from '../library/library-model';
import { nameOf } from '../library/use-row-actions';
import { whenLabel } from '../library/when-label';
import { EmptyConfirm } from './empty-confirm';
import { daysLeft, RETENTION_DAYS, SOON_DAYS } from './trash-model';
import './trash.css';

const PAGE_LIMIT = 500;
/** A vault snippet in the trash is still encrypted, and still shown masked. */
const MASK = '•••••••••••';
const rowId = (index: number) => `tvt-row-${String(index)}`;

/**
 * The trash: nothing deleted disappears at once. Every row says how many days
 * it has left, amber once that is under a week, and "Restore" is on the row.
 * Emptying the whole trash is the one place the desktop asks first.
 */
export function TrashPage() {
  const tr = useTr();
  const locale = useLocale();
  const { notifyMutation } = useEspanso();
  const { say } = useUndo();
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [pick, setPick] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const menu = usePaperMenu(tr('Trash actions', '回收站操作'));

  const reload = useCallback(() => {
    listTrash(PAGE_LIMIT, 0)
      .then((trashed) => {
        setRows(trashed);
        setFailed(false);
      })
      .catch(() => setFailed(true));
    fetchFolders()
      .then(setFolders)
      .catch(() => setFolders([]));
  }, []);

  useEffect(() => {
    // The 30-day rule is enforced on the way in; expired rows never render.
    purgeExpiredTrash()
      .catch(() => {
        // A failed purge must not keep the list from showing.
      })
      .finally(reload);
  }, [reload]);
  useLibraryChange(reload);

  const now = Date.now();
  const at = rows === null || rows.length === 0 ? -1 : Math.min(pick, rows.length - 1);
  const picked = at < 0 ? undefined : rows?.[at];
  const count = rows?.length ?? 0;

  const vaulted = (snippet: Snippet) => snippet.securityLevel === 'sensitive';
  const collectionOf = (snippet: Snippet) =>
    vaulted(snippet)
      ? tr('Vault', '保险库')
      : (folders.find((folder) => folder.id === snippet.folderId)?.name ??
        tr('Unsorted', '待整理'));

  const restore = (snippet: Snippet) => {
    const name = nameOf(snippet, tr);
    restoreSnippet(snippet.id)
      .then(() => {
        notifyMutation();
        announceLibraryChange();
        say({
          mood: 'happy',
          title: tr(`${name} is back.`, `找回了 ${name}。`),
          body: vaulted(snippet)
            ? tr('It is in the vault again.', '它回到了保险库。')
            : tr(
                `It is in “${collectionOf(snippet)}” again.`,
                `它回到了「${collectionOf(snippet)}」。`,
              ),
        });
      })
      .catch(() =>
        say({
          title: tr(`${name} didn't come back.`, `${name} 没找回来。`),
          body: tr('It is still here in the trash.', '它还在回收站里。'),
        }),
      );
  };

  const purge = (snippet: Snippet) => {
    const name = nameOf(snippet, tr);
    deleteSnippetForever(snippet.id)
      .then(() => {
        announceLibraryChange();
        say({
          mood: null,
          title: tr(`${name} is gone for good.`, `${name} 彻底删掉了。`),
          body: tr('The trash cannot bring it back now.', '回收站里也找不回它了。'),
        });
      })
      .catch(() =>
        say({
          title: tr(`${name} wasn't deleted.`, `${name} 没删掉。`),
          body: tr('It is still in the trash.', '它还在回收站里。'),
        }),
      );
  };

  const emptyTrash = () => {
    const ids = (rows ?? []).map((row) => row.id);
    setConfirming(false);
    void Promise.allSettled(ids.map((id) => deleteSnippetForever(id))).then((results) => {
      announceLibraryChange();
      const left = String(results.filter((result) => result.status === 'rejected').length);
      if (left !== '0') {
        say({
          title: tr(`${left} are still in the trash.`, `还有 ${left} 条没清掉。`),
          body: tr(
            'The rest are gone; empty it again to finish.',
            '其余的已经清掉了,可以再清一次。',
          ),
        });
      }
    });
  };

  const entriesFor = (snippet: Snippet): MenuEntry[] => [
    { kind: 'item', label: tr('Restore', '找回'), keys: '⏎', onSelect: () => restore(snippet) },
    { kind: 'separator' },
    {
      kind: 'item',
      label: tr('Delete for good', '彻底删掉'),
      danger: true,
      onSelect: () => purge(snippet),
    },
  ];
  const openMenu = (point: MenuPoint, index: number, snippet: Snippet) => {
    setPick(index);
    menu.open(point, entriesFor(snippet));
  };

  useCommand('trash.restore-picked', () => picked !== undefined && restore(picked));
  useCommand('trash.purge-picked', () => picked !== undefined && purge(picked));
  useCommand('trash.empty', () => setConfirming(true));

  const onKeyDown = (event: KeyboardEvent) => {
    if (rows === null || rows.length === 0) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      setPick(Math.min(Math.max(at + step, 0), rows.length - 1));
    } else if (event.key === 'Enter' && picked !== undefined) {
      event.preventDefault();
      restore(picked);
    } else if (
      picked !== undefined &&
      (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))
    ) {
      event.preventDefault();
      const rect = document.getElementById(rowId(at))?.getBoundingClientRect();
      openMenu({ x: (rect?.left ?? 0) + 96, y: rect?.bottom ?? 0 }, at, picked);
    }
  };

  if (rows !== null && rows.length === 0 && !failed) {
    return (
      <div className="tpi tvt">
        <div className="tvt-sheet tvt-empty-state">
          <div className="tpi-eyebrow">{tr('Trash · empty', '回收站 · 空')}</div>
          <div className="tvt-empty-line">
            <Mascot size={44} />
            <h1 className="tvt-empty-title">{tr('The trash is empty.', '回收站是空的。')}</h1>
          </div>
          <p className="tvt-empty-body">
            {tr(
              `Deleted snippets wait here for ${String(RETENTION_DAYS)} days. There are none right now.`,
              `删掉的片段会在这里待 ${String(RETENTION_DAYS)} 天。现在一条都没有。`,
            )}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="tpi tvt">
      <div className="tvt-sheet">
        <div className="tvt-head">
          <h1>{tr('Trash', '回收站')}</h1>
          {rows !== null && (
            <span className="tvt-count">{tr(String(count), `${String(count)} 条`)}</span>
          )}
        </div>
        <p className="tvt-lede">
          {tr(
            `Deleted snippets are cleared after ${String(RETENTION_DAYS)} days. Until then any of them can come back, with its trigger and use count.`,
            `删掉 ${String(RETENTION_DAYS)} 天后自动清掉。在这之前随时能找回,触发词和用过多少次都还在。`,
          )}
        </p>

        {failed ? (
          <div className="tvt-state">
            <Mascot state="confused" size={42} />
            <p>
              {tr(
                'The trash list did not load just now; nothing in it was touched.',
                '回收站刚才没读出来,里面的东西一点没动。',
              )}
            </p>
            <button type="button" className="tpi-note-yes" onClick={reload}>
              {tr('Read it again', '再读一次')}
            </button>
          </div>
        ) : rows === null ? (
          <div className="tvt-state" role="status">
            <Mascot state="thinking" size={42} />
            <p>{tr('Reading the trash', '正在读回收站')}</p>
          </div>
        ) : (
          <div
            className="tvt-rows"
            role="listbox"
            tabIndex={0}
            aria-label={tr('Deleted snippets', '删掉的片段')}
            aria-activedescendant={at < 0 ? undefined : rowId(at)}
            onKeyDown={onKeyDown}
          >
            {rows.map((snippet, index) => {
              const left =
                snippet.deletedAt === null ? RETENTION_DAYS : daysLeft(snippet.deletedAt, now);
              const when =
                snippet.deletedAt === null ? '' : whenLabel(snippet.deletedAt, now, locale);
              const meta = [
                collectionOf(snippet),
                tr(`deleted ${when}`, `${when} 删掉`),
                ...(vaulted(snippet) ? [tr('still encrypted', '内容仍是加密的')] : []),
              ].join(' · ');
              return (
                <div
                  key={snippet.id}
                  id={rowId(index)}
                  role="option"
                  aria-selected={index === at}
                  className="tvt-row"
                  onClick={() => setPick(index)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    openMenu({ x: event.clientX, y: event.clientY }, index, snippet);
                  }}
                >
                  <span className="tvt-trigger">{snippet.trigger ?? '—'}</span>
                  <span className="tvt-main">
                    <span className="tvt-body">{vaulted(snippet) ? MASK : firstLine(snippet)}</span>
                    <span className="tvt-meta">{meta}</span>
                  </span>
                  {left < SOON_DAYS && <span aria-hidden="true" className="tvt-soon" />}
                  <span className="tvt-left">
                    {tr(`${String(left)} days left`, `还剩 ${String(left)} 天`)}
                  </span>
                  <button
                    type="button"
                    className="tvt-restore"
                    onClick={(event) => {
                      event.stopPropagation();
                      restore(snippet);
                    }}
                  >
                    {tr('Restore', '找回')}
                  </button>
                </div>
              );
            })}
          </div>
        )}

        {count > 0 && (
          <>
            <div className="tvt-rule" />
            <div className="tvt-foot">
              <button type="button" className="tvt-destroy" onClick={() => setConfirming(true)}>
                {tr('Empty the trash', '清空回收站')}
              </button>
              <span className="tvt-foot-note">
                {tr('It asks once before it does.', '清空之前会问一次。')}
              </span>
            </div>
          </>
        )}
      </div>
      {confirming && count > 0 && (
        <EmptyConfirm count={count} onConfirm={emptyTrash} onCancel={() => setConfirming(false)} />
      )}
      {menu.node}
    </div>
  );
}
