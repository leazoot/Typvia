// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { listen } from '@tauri-apps/api/event';
import {
  copySnippet,
  hidePanel,
  IpcError,
  panelCopySecret,
  panelInsert,
  panelInsertSecret,
  panelInsertTemplate,
  panelReady,
  panelResults,
  templateFields,
  vaultStatus,
} from '@typvia/shared';
import type { Snippet, TemplateField } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { KeyCap } from '../../paper/kit';
import { loadInsertMethod } from '../../workspace/insert-method';
import { PanelFill } from './panel-fill';
import { PanelRow } from './panel-row';
import { PanelVerify } from './panel-verify';
import './panel.css';

/** Window label of the resident panel (must match tauri.conf.json). */
export const PANEL_WINDOW_LABEL = 'panel';

/** Max rows fetched for the recent list and each search (panel is a shortlist). */
const RESULT_LIMIT = 20;

/** Rows that get a ⌘-digit of their own. */
const DIGIT_ROWS = 9;

/** Focuses the search input; the resident WebView must re-focus on every show. */
function focusSearch(root: HTMLElement | null): void {
  root?.querySelector('input')?.focus();
}

/**
 * Quick Bar. The search line is focused at frame 0 — you type through the
 * enter animation. Summoned by ⌘⇧V; the empty query shows recent snippets,
 * typing replaces them with ranked results (no debounce). ↑↓ moves, ⏎ inserts
 * into the app you came from, ⌥⏎ only copies, ⇥ opens a template's variables,
 * ⌘1–⌘9 insert a row directly. Hides on esc or blur.
 */
export function PanelApp() {
  const tr = useTr();
  const [query, setQuery] = useState('');
  // Bumped on every summon so the enter animation replays and the field
  // re-focuses (the resident WebView is never torn down between shows).
  const [showId, setShowId] = useState(0);
  const [destination, setDestination] = useState<string | null>(null);
  const [results, setResults] = useState<Snippet[]>([]);
  // Rows an app rule hid for the destination app — stated, never silent.
  const [hiddenByRules, setHiddenByRules] = useState(0);
  // Set when an insert was refused by an app rule; cleared on typing/summon.
  const [ruleNotice, setRuleNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState(0);
  // Non-null while filling a template's fields before injection.
  const [filling, setFilling] = useState<{ snippet: Snippet; fields: TemplateField[] } | null>(
    null,
  );
  // Non-null while a sensitive call waits on a vault unlock.
  const [verifying, setVerifying] = useState<{
    snippet: Snippet;
    action: 'insert' | 'copy';
  } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  // Guards against out-of-order responses: only the latest load may apply.
  const loadSeq = useRef(0);

  const load = useCallback((q: string) => {
    const seq = (loadSeq.current += 1);
    // The host resolves recent-vs-search and filters by the destination
    // app's rules, returning how many rows that hid.
    panelResults(q, RESULT_LIMIT)
      .then((page) => {
        if (loadSeq.current === seq) {
          setResults(page.rows);
          setHiddenByRules(page.hiddenByRules);
          setSelected(0);
        }
      })
      .catch(() => {
        if (loadSeq.current === seq) {
          setResults([]);
          setHiddenByRules(0);
          setSelected(0);
        }
      });
  }, []);

  useEffect(() => {
    const pending = listen<{ destination: string | null }>('panel:show', (event) => {
      setDestination(event.payload.destination);
      setQuery('');
      setFilling(null);
      setVerifying(null);
      setRuleNotice(null);
      load('');
      setShowId((id) => id + 1);
    });
    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, [load]);

  // Runs on first mount and after every summon: focus the field, then report
  // first paint for the latency budget (debug-only on the Rust side).
  useEffect(() => {
    focusSearch(rootRef.current);
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (import.meta.env.DEV) void panelReady();
      });
    });
  }, [showId]);

  // Keep the selected row in view as arrow keys walk the list.
  useEffect(() => {
    const row = rootRef.current?.querySelector('[data-selected="true"]');
    if (row && typeof row.scrollIntoView === 'function') {
      row.scrollIntoView({ block: 'nearest' });
    }
  }, [selected]);

  const onQueryChange = (value: string) => {
    setQuery(value);
    setRuleNotice(null);
    load(value);
  };

  // Rule-block copy: what is still fine first, then what stands in the way.
  const ruleBlockedNotice = () =>
    tr(
      `Your snippet is safe — an app rule blocks inserting into ${destination ?? 'this app'}.`,
      `你的片段仍然安全——应用规则拦截了向 ${destination ?? '此应用'} 的插入。`,
    );

  // Sensitive snippets deliver through the vault: verify the session is
  // unlocked (prompt if not), then inject/copy host-side — the plaintext never
  // crosses IPC. A session that lapsed between the check and the call surfaces
  // permission_denied and re-prompts.
  const performSecret = (snippet: Snippet, action: 'insert' | 'copy') => {
    setVerifying(null);
    if (action === 'insert') {
      panelInsertSecret(snippet.id, loadInsertMethod()).catch((error: unknown) => {
        if (error instanceof IpcError && error.code === 'permission_denied') {
          setVerifying({ snippet, action: 'insert' });
        } else if (error instanceof IpcError && error.code === 'rule_blocked') {
          setRuleNotice(ruleBlockedNotice());
        }
      });
    } else {
      panelCopySecret(snippet.id)
        .then(() => {
          void hidePanel();
        })
        .catch((error: unknown) => {
          if (error instanceof IpcError && error.code === 'permission_denied') {
            setVerifying({ snippet, action: 'copy' });
          } else {
            void hidePanel();
          }
        });
    }
  };

  const runSecret = (snippet: Snippet, action: 'insert' | 'copy') => {
    void vaultStatus()
      .then((status) => {
        if (status.unlocked) performSecret(snippet, action);
        else setVerifying({ snippet, action });
      })
      .catch(() => setVerifying({ snippet, action }));
  };

  const insert = (snippet: Snippet) => {
    // Templates enter fill mode first: load their fields, then either inject
    // straight through (no variables) or show the fill form.
    if (snippet.snippetType === 'template') {
      void templateFields(snippet.id)
        .then((fields) => {
          if (fields.length === 0) {
            void panelInsertTemplate(snippet.id, {}, loadInsertMethod()).catch((error: unknown) => {
              if (error instanceof IpcError && error.code === 'rule_blocked') {
                setRuleNotice(ruleBlockedNotice());
              }
            });
          } else {
            setFilling({ snippet, fields });
          }
        })
        .catch(() => undefined);
      return;
    }
    // Sensitive snippets go through the vault verify/deliver path.
    if (snippet.securityLevel === 'sensitive') {
      runSecret(snippet, 'insert');
      return;
    }
    // The host hides the panel and restores focus before injecting; a denied
    // permission falls back to copying so the action is never silently lost.
    // A rule block happens before the hide, so the panel stays up to say so.
    panelInsert(snippet.id, loadInsertMethod()).catch((error: unknown) => {
      if (error instanceof IpcError && error.code === 'permission_denied') {
        void copySnippet(snippet.id);
      } else if (error instanceof IpcError && error.code === 'rule_blocked') {
        setRuleNotice(ruleBlockedNotice());
      }
    });
  };

  const copy = (snippet: Snippet) => {
    if (snippet.securityLevel === 'sensitive') {
      runSecret(snippet, 'copy');
      return;
    }
    void copySnippet(snippet.id).finally(() => {
      void hidePanel();
    });
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    // In fill/verify mode the sub-view owns the keyboard (⏎ acts, esc backs).
    if (filling !== null || verifying !== null) return;
    const picked = results[selected];
    if (event.key === 'Escape') {
      event.preventDefault();
      void hidePanel();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelected((index) => Math.min(index + 1, Math.max(results.length - 1, 0)));
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelected((index) => Math.max(index - 1, 0));
      return;
    }
    if (event.key === 'Tab') {
      // ⇥ fills variables; on anything else it stays in the search line.
      event.preventDefault();
      if (picked?.snippetType === 'template') insert(picked);
      return;
    }
    if ((event.metaKey || event.ctrlKey) && /^[1-9]$/.test(event.key)) {
      const row = results[Number(event.key) - 1];
      if (row === undefined) return;
      event.preventDefault();
      insert(row);
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      if (picked === undefined) return;
      if (event.altKey || event.shiftKey) copy(picked);
      else insert(picked);
    }
  };

  return (
    <div ref={rootRef} className="tpi tvq" onKeyDown={onKeyDown}>
      <div key={showId} className="tvq-card">
        {filling !== null ? (
          <PanelFill
            snippet={filling.snippet}
            fields={filling.fields}
            destination={destination}
            onCancel={() => setFilling(null)}
          />
        ) : verifying !== null ? (
          <PanelVerify
            snippet={verifying.snippet}
            action={verifying.action}
            destination={destination}
            onUnlocked={() => performSecret(verifying.snippet, verifying.action)}
            onCancel={() => setVerifying(null)}
          />
        ) : (
          <>
            <div className="tvq-search">
              <svg width="15" height="15" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                <circle cx="6" cy="6" r="4.6" stroke="currentColor" strokeWidth="1.3" />
                <path
                  d="M9.6 9.6L13 13"
                  stroke="currentColor"
                  strokeWidth="1.3"
                  strokeLinecap="round"
                />
              </svg>
              <input
                className="tvq-input"
                value={query}
                spellCheck={false}
                autoComplete="off"
                aria-label={tr('Search snippets', '搜索片段')}
                placeholder={tr('Search snippets', '搜索片段')}
                onChange={(event) => onQueryChange(event.target.value)}
              />
              <span className="tvq-count" aria-live="polite">
                {query.trim() === ''
                  ? tr(`${String(results.length)} recent`, `最近 ${String(results.length)} 条`)
                  : tr(`${String(results.length)} found`, `${String(results.length)} 条`)}
              </span>
            </div>

            <div className="tvq-list" role="listbox" aria-label={tr('Snippets', '片段')}>
              {results.length === 0 ? (
                <p className="tvq-empty">
                  {query.trim() === ''
                    ? tr('No snippets yet', '还没有片段')
                    : tr('No matches', '没有匹配结果')}
                </p>
              ) : (
                results.map((snippet, index) => (
                  <PanelRow
                    key={snippet.id}
                    snippet={snippet}
                    selected={index === selected}
                    shortcut={index < DIGIT_ROWS ? `⌘${String(index + 1)}` : null}
                    onHover={() => setSelected(index)}
                    onPick={() => insert(snippet)}
                  />
                ))
              )}
              {hiddenByRules > 0 && (
                <p className="tvq-hidden">
                  {tr(
                    `${hiddenByRules} hidden by app rules${destination !== null ? ` for ${destination}` : ''}`,
                    destination !== null
                      ? `应用规则为 ${destination} 隐藏了 ${hiddenByRules} 条`
                      : `应用规则隐藏了 ${hiddenByRules} 条`,
                  )}
                </p>
              )}
            </div>

            {ruleNotice !== null && (
              <p className="tvq-notice" role="status">
                {ruleNotice}
              </p>
            )}

            <footer className="tvq-foot">
              <KeyCap>⏎</KeyCap>
              <span className="tvq-foot-word">{tr('insert', '插入')}</span>
              <KeyCap>⌥⏎</KeyCap>
              <span className="tvq-foot-word">{tr('copy only', '只复制')}</span>
              <KeyCap>⇥</KeyCap>
              <span className="tvq-foot-word">{tr('fill variables', '填变量')}</span>
              <span className="tvq-spacer" />
              <span className="tvq-foot-note">
                {destination !== null
                  ? `→ ${destination}`
                  : tr('Just opened · press ⏎', '刚打开 · 直接按 ⏎')}
              </span>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}
