// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listen } from '@tauri-apps/api/event';
import {
  autostartStatus,
  hideTray,
  insertionPauseStatus,
  libraryCounts,
  pauseInsertion,
  quitApp,
  resumeInsertion,
  setAutostart,
  syncStatus,
  trayInsert,
  trayOpenLibrary,
  trayPresent,
  trayResults,
  vaultLock,
  vaultStatus,
} from '@typvia/shared';
import type { InsertionPause, Snippet, SyncStatus, VaultStatus } from '@typvia/shared';
import { useLocale, useTr, type Tr } from '@typvia/ui';
import { Fragment, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { CheckBox, KeyCap } from '../../paper/kit';
import { loadInsertMethod } from '../../workspace/insert-method';
import { previewLine } from '../panel/panel-row';
import './tray.css';

/** Window label of the tray card (must match tauri.conf.json). */
export const TRAY_WINDOW_LABEL = 'tray';

/** Snippets the card offers to insert straight away. */
const ROWS = 4;
const PAUSE_MINUTES = 60;

const IS_MAC = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');
/** Start-at-login lives in the Windows tray menu; macOS keeps it in System Settings. */
const OFFERS_STARTUP = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');

interface Facts {
  sync: SyncStatus | null;
  total: number | null;
  rows: Snippet[];
  vault: VaultStatus | null;
  pause: InsertionPause;
  startup: boolean | null;
}

function syncWord(sync: SyncStatus | null, tr: Tr): { word: string; tone: string } | null {
  if (sync === null || !sync.configured) return null;
  if (!sync.enabled) return { word: tr('Paused', '已暂停'), tone: 'off' };
  if (sync.pendingBacklog > 0 || sync.lastSyncAt === null) {
    return { word: tr('Not synced', '未同步'), tone: 'warn' };
  }
  return { word: tr('Synced', '已同步'), tone: 'ok' };
}

function settled<T>(result: PromiseSettledResult<T>, fallback: T): T {
  return result.status === 'fulfilled' ? result.value : fallback;
}

/**
 * The card the menu-bar / tray icon opens: how sync stands, the most recent
 * snippets to insert straight away, and the few things worth doing without
 * opening the window. It lays itself out first and then asks the host to
 * place and show it, so it never appears at a stale size.
 */
export function TrayApp() {
  const tr = useTr();
  const locale = useLocale();
  const [facts, setFacts] = useState<Facts | null>(null);
  const [shown, setShown] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  const load = useCallback(() => {
    void Promise.allSettled([
      syncStatus(),
      libraryCounts(),
      trayResults(ROWS),
      vaultStatus(),
      insertionPauseStatus(),
      OFFERS_STARTUP ? autostartStatus() : Promise.resolve(null),
    ]).then(([sync, counts, rows, vault, pause, startup]) => {
      setFacts({
        sync: settled(sync, null),
        total: counts.status === 'fulfilled' ? counts.value.total : null,
        rows: settled(rows, []),
        vault: settled(vault, null),
        pause: settled(pause, { remainingMs: null }),
        startup: settled(startup, null),
      });
      setNotice(null);
      setShown((count) => count + 1);
    });
  }, []);

  useEffect(() => {
    const pending = listen('tray:show', load);
    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, [load]);

  // Every layout change after a show — rows, a lock that went away, a
  // notice — re-measures the card so the window keeps hugging it.
  useLayoutEffect(() => {
    if (shown === 0) return;
    const height = cardRef.current?.getBoundingClientRect().height ?? 0;
    if (height > 0) void trayPresent(Math.ceil(height));
  }, [shown, facts, notice]);

  const failed = useCallback(() => setNotice(tr('That did not work', '没成功')), [tr]);

  const insert = useCallback(
    (snippet: Snippet) => {
      trayInsert(snippet.id, loadInsertMethod()).catch(() =>
        setNotice(tr('Not inserted', '没插进去')),
      );
    },
    [tr],
  );

  const rows = facts?.rows ?? [];
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        void hideTray();
        return;
      }
      const chord = IS_MAC
        ? event.metaKey && !event.altKey && !event.ctrlKey
        : event.ctrlKey && event.altKey;
      const digit = /^Digit([1-9])$/.exec(event.code)?.[1];
      const row = chord && digit !== undefined ? rows[Number(digit) - 1] : undefined;
      if (row !== undefined) {
        event.preventDefault();
        insert(row);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [rows, insert]);

  const patch = (changes: Partial<Facts>) =>
    setFacts((current) => (current === null ? current : { ...current, ...changes }));

  const status = syncWord(facts?.sync ?? null, tr);
  const lastSync = facts?.sync?.configured ? facts.sync.lastSyncAt : null;
  const parts = [
    status?.word,
    facts?.total == null
      ? undefined
      : tr(`${String(facts.total)} snippets`, `${String(facts.total)} 条`),
    lastSync == null
      ? undefined
      : new Date(lastSync).toLocaleTimeString(locale === 'en' ? 'en-US' : 'zh-CN', {
          hour: '2-digit',
          minute: '2-digit',
          hour12: false,
        }),
  ].filter((part): part is string => part !== undefined);

  const remaining = facts?.pause.remainingMs ?? null;
  const minutesLeft = remaining === null ? '' : String(Math.max(1, Math.ceil(remaining / 60_000)));

  return (
    <div className="tvr" data-chrome={OFFERS_STARTUP ? 'windows' : 'mac'}>
      <div ref={cardRef} className="tvr-card">
        {parts.length > 0 && (
          <p className="tvr-status">
            {status !== null && (
              <span aria-hidden="true" className="tvr-dot" data-tone={status.tone} />
            )}
            {parts.map((part, index) => (
              <Fragment key={part}>
                {index > 0 && (
                  <span aria-hidden="true" className="tvr-sep">
                    ·
                  </span>
                )}
                <span>{part}</span>
              </Fragment>
            ))}
          </p>
        )}
        {rows.length > 0 && (
          <ul className="tvr-rows">
            {rows.map((snippet, index) => (
              <li key={snippet.id}>
                <button type="button" className="tvr-row" onClick={() => insert(snippet)}>
                  <span className="tvr-row-words">
                    {previewLine(snippet.body) || snippet.title}
                  </span>
                  <KeyCap>
                    {IS_MAC ? `⌘${String(index + 1)}` : `Ctrl Alt ${String(index + 1)}`}
                  </KeyCap>
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="tvr-actions">
          <button type="button" className="tvr-action" onClick={() => void trayOpenLibrary()}>
            {tr('Open the library', '打开资料库')}
          </button>
          <button
            type="button"
            className="tvr-action"
            onClick={() => {
              const next = remaining === null ? pauseInsertion(PAUSE_MINUTES) : resumeInsertion();
              next.then((pause) => patch({ pause })).catch(failed);
            }}
          >
            {remaining === null
              ? tr('Pause inserting for 1 hour', '暂停插入 1 小时')
              : tr(
                  `Resume inserting · ${minutesLeft} min left`,
                  `恢复插入 · 还剩 ${minutesLeft} 分钟`,
                )}
          </button>
          {facts?.vault?.unlocked === true && (
            <button
              type="button"
              className="tvr-action"
              onClick={() => {
                vaultLock()
                  .then((vault) => patch({ vault }))
                  .catch(failed);
              }}
            >
              {tr('Lock the vault', '锁上保险库')}
            </button>
          )}
          {OFFERS_STARTUP && facts?.startup != null && (
            <div className="tvr-check">
              <CheckBox
                checked={facts.startup}
                onChange={(enabled) => {
                  setAutostart(enabled)
                    .then((startup) => patch({ startup }))
                    .catch(failed);
                }}
              >
                {tr('Start at login', '开机启动')}
              </CheckBox>
            </div>
          )}
          <button type="button" className="tvr-action is-quiet" onClick={() => void quitApp()}>
            {tr('Quit Typvia', '退出 Typvia')}
          </button>
        </div>
        {notice !== null && (
          <p className="tvr-notice" role="status">
            {notice}
          </p>
        )}
      </div>
    </div>
  );
}
