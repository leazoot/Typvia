// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  panelCopySecret,
  restoreSnippet,
  type Snippet,
  trashSnippet,
  vaultCreateSecret,
  vaultList,
  vaultReveal,
  type VaultStatus,
  vaultUpdateSecret,
} from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { GroupTitle } from '../../paper/kit';
import { usePaperMenu, type MenuEntry } from '../../paper/menu';
import { useCommand } from '../../workspace/commands';
import { announceLibraryChange } from '../../workspace/library-events';
import { useUndo } from '../../workspace/undo';
import { SecretForm, SecretRow } from './secret-row';
import { TouchIdOffer, VaultPolicy } from './touch-id';

/** A peeked secret covers itself again after this many seconds. */
const PEEK_SECONDS = 10;
/** Past this many secrets a search line appears; below it the list is the search. */
const SEARCH_FROM = 8;

function useRelockClock(status: VaultStatus): { minutes: number; seconds: number } {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, []);
  const base = status.lastActivityAt ?? status.unlockedAt ?? now;
  const left = Math.max(0, base + status.idleTimeoutMs - now);
  return { minutes: Math.floor(left / 60_000), seconds: Math.floor((left % 60_000) / 1_000) };
}

/** One secret on screen at a time, and never for longer than it was asked for. */
function usePeek() {
  const [shown, setShown] = useState<{ id: string; text: string; left: number } | null>(null);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  const cover = useCallback(() => {
    if (timer.current !== null) clearInterval(timer.current);
    timer.current = null;
    setShown(null);
  }, []);
  useEffect(() => cover, [cover]);
  useEffect(() => {
    if (shown !== null && shown.left <= 0) cover();
  }, [shown, cover]);

  const peek = useCallback(
    async (id: string) => {
      cover();
      const text = await vaultReveal(id);
      setShown({ id, text, left: PEEK_SECONDS });
      timer.current = setInterval(
        () =>
          setShown((current) => (current === null ? null : { ...current, left: current.left - 1 })),
        1_000,
      );
    },
    [cover],
  );
  return { shown, peek, cover };
}

export function VaultUnlocked({
  status,
  onLock,
  startCreating,
}: {
  status: VaultStatus;
  onLock: () => Promise<void>;
  startCreating: boolean;
}) {
  const tr = useTr();
  const { offer, say } = useUndo();
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState('');
  const [creating, setCreating] = useState(startCreating);
  const [editing, setEditing] = useState<{ id: string; title: string; body: string } | null>(null);
  const [focused, setFocused] = useState<string | null>(null);
  const [copied, setCopied] = useState<{ id: string; seconds: number } | null>(null);
  const peek = usePeek();
  const menu = usePaperMenu(tr('Secret actions', '密文操作'));
  const { minutes, seconds } = useRelockClock(status);

  const load = useCallback(async () => {
    try {
      setRows(await vaultList(500, 0));
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (copied === null) return;
    const timer = setTimeout(() => setCopied(null), 3_000);
    return () => clearTimeout(timer);
  }, [copied]);

  const filtered = useMemo(() => {
    if (rows === null) return null;
    const needle = query.trim().toLowerCase();
    return needle === '' ? rows : rows.filter((row) => row.title.toLowerCase().includes(needle));
  }, [rows, query]);

  const didNot = (title: string) =>
    say({
      title,
      body: tr(
        'Everything in the vault is still encrypted and unchanged.',
        '保险库里的东西都还加密着,没有变。',
      ),
    });

  const peekAt = (row: Snippet) =>
    peek
      .peek(row.id)
      .catch(() => didNot(tr(`“${row.title}” didn't open.`, `「${row.title}」没打开。`)));
  const copy = (row: Snippet) =>
    panelCopySecret(row.id)
      .then((ms) => setCopied({ id: row.id, seconds: Math.round(ms / 1_000) }))
      .catch(() => didNot(tr(`“${row.title}” wasn't copied.`, `「${row.title}」没复制上。`)));
  const edit = (row: Snippet) => {
    peek.cover();
    vaultReveal(row.id)
      .then((body) => setEditing({ id: row.id, title: row.title, body }))
      .catch(() => didNot(tr(`“${row.title}” didn't open.`, `「${row.title}」没打开。`)));
  };
  const remove = (row: Snippet) =>
    trashSnippet(row.id)
      .then(() => {
        void load();
        announceLibraryChange();
        offer({
          title: tr(`Deleted “${row.title}”.`, `已删掉「${row.title}」。`),
          body: tr(
            '⌘Z takes it back, or find it in the trash within 30 days — still encrypted.',
            '⌘Z 撤回,或者 30 天内在回收站里找回 —— 仍是加密的。',
          ),
          failure: tr('It is in the trash, still encrypted.', '它在回收站里,仍是加密的。'),
          undo: () =>
            restoreSnippet(row.id).then(() => {
              void load();
              announceLibraryChange();
            }),
        });
      })
      .catch(() => didNot(tr(`“${row.title}” wasn't deleted.`, `「${row.title}」没删掉。`)));

  const entriesFor = (row: Snippet): MenuEntry[] => [
    { kind: 'item', label: tr('Peek', '看一眼'), keys: '⌥⌘L', onSelect: () => void peekAt(row) },
    { kind: 'item', label: tr('Copy', '复制'), onSelect: () => void copy(row) },
    { kind: 'item', label: tr('Edit', '编辑'), onSelect: () => edit(row) },
    { kind: 'separator' },
    {
      kind: 'item',
      label: tr('Delete', '删除'),
      danger: true,
      onSelect: () => void remove(row),
    },
  ];

  useCommand('vault.peek', () => {
    const row = filtered?.find((entry) => entry.id === focused) ?? filtered?.[0];
    if (row !== undefined) void peekAt(row);
  });

  const m = String(minutes);
  const clock = `${m}:${String(seconds).padStart(2, '0')}`;

  return (
    <>
      <section className="tvv-card is-open" aria-label={tr('Vault', '保险库')}>
        <div className="tvv-open-head">
          <GroupTitle
            trailing={
              <span className="tvv-head-actions">
                <button
                  type="button"
                  className="tvv-word"
                  aria-expanded={creating}
                  onClick={() => setCreating((open) => !open)}
                >
                  {tr('New secret', '新建密文')}
                </button>
                <button type="button" className="tvv-word" onClick={() => void onLock()}>
                  {tr('Lock', '锁上')}
                </button>
              </span>
            }
          >
            {tr(`Unlocked ${clock}`, `已解锁 ${clock}`)}
          </GroupTitle>
        </div>

        {creating && (
          <SecretForm
            submitLabel={tr('Save secret', '存好密文')}
            onCancel={() => setCreating(false)}
            onSave={async (title, body) => {
              await vaultCreateSecret({
                title,
                body,
                snippetType: 'sensitive',
                description: null,
                folderId: null,
                trigger: null,
                triggerMode: null,
                language: null,
              });
              setCreating(false);
              await load();
              announceLibraryChange();
            }}
          />
        )}

        {rows !== null && rows.length >= SEARCH_FROM && (
          <label className="tvv-search">
            <input
              type="text"
              value={query}
              aria-label={tr('Search the vault by title', '按标题找密文')}
              placeholder={tr(
                'Titles only — contents are never searched',
                '只搜标题 —— 内容从不参与搜索',
              )}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
        )}

        {failed ? (
          <p className="tvv-text">
            {tr(
              'Your secrets are safe and encrypted on this Mac; the list just did not load.',
              '你的密文都安全地加密在这台 Mac 上,只是列表刚才没读出来。',
            )}
          </p>
        ) : filtered === null ? null : filtered.length === 0 ? (
          <p className="tvv-text">
            {query.trim() === ''
              ? tr(
                  'The vault is empty. What you add is encrypted before it is saved, and never indexed.',
                  '保险库还是空的。放进来的东西先加密再保存,也从不进索引。',
                )
              : tr(
                  'No secret has that title. Contents are never searched.',
                  '没有这个标题的密文。内容从不参与搜索。',
                )}
          </p>
        ) : (
          <div className="tvv-rows" role="list">
            {filtered.map((row) =>
              editing?.id === row.id ? (
                <SecretForm
                  key={row.id}
                  initial={editing}
                  submitLabel={tr('Save', '存好')}
                  onCancel={() => setEditing(null)}
                  onSave={async (title, body) => {
                    await vaultUpdateSecret({
                      id: row.id,
                      title,
                      body,
                      snippetType: row.snippetType,
                      description: row.description,
                      folderId: row.folderId,
                      trigger: row.trigger,
                      triggerMode: row.triggerMode,
                      language: row.language,
                      isFavorite: row.isFavorite,
                      isPinned: row.isPinned,
                      isEnabled: row.isEnabled,
                    });
                    setEditing(null);
                    await load();
                  }}
                />
              ) : (
                <div key={row.id} onFocus={() => setFocused(row.id)}>
                  <SecretRow
                    row={row}
                    shown={peek.shown?.id === row.id ? peek.shown : null}
                    copiedSeconds={copied?.id === row.id ? copied.seconds : null}
                    onPeek={() => void peekAt(row)}
                    onCover={peek.cover}
                    onCopy={() => void copy(row)}
                    onEdit={() => edit(row)}
                    onMenu={(event) => {
                      event.preventDefault();
                      setFocused(row.id);
                      menu.open({ x: event.clientX, y: event.clientY }, entriesFor(row));
                    }}
                  />
                </div>
              ),
            )}
          </div>
        )}

        <div className="tvv-foot">
          <span aria-hidden="true" className="tvv-warn-dot" />
          <span>
            {tr(
              `Locks itself in ${m} min ${String(seconds)} s · a copied secret clears from the clipboard by itself`,
              `还有 ${m} 分 ${String(seconds)} 秒自动重锁 · 复制出去的会自己从剪贴板清掉`,
            )}
          </span>
        </div>
      </section>
      <TouchIdOffer />
      <VaultPolicy minutes={Math.max(1, Math.round(status.idleTimeoutMs / 60_000))} />
      {menu.node}
    </>
  );
}
