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
import { SearchLine, TypeMark, markForType, useTr } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { PanelFill } from './panel-fill';
import { PanelVerify } from './panel-verify';
import './panel.css';

/** Window label of the resident panel (must match tauri.conf.json). */
export const PANEL_WINDOW_LABEL = 'panel';

/** Max rows fetched for the recent list and each search (panel is a shortlist). */
const RESULT_LIMIT = 20;

/** Focuses the search input; the resident WebView must re-focus on every show. */
function focusSearch(root: HTMLElement | null): void {
  root?.querySelector('input')?.focus();
}

/** First non-empty line of the body — the row preview (kept aria-hidden).
 * Sensitive snippets carry a null body (their plaintext never crosses IPC). */
function previewLine(body: string | null): string {
  if (body === null) return '';
  return (
    body
      .split('\n')
      .find((line) => line.trim() !== '')
      ?.trim() ?? ''
  );
}

/**
 * Global command panel. The search
 * field is focused at frame 0 — you type through the enter animation. Summoned
 * by ⌘⇧V; the empty query shows recent snippets, typing replaces them with
 * ranked results (no debounce). ↑↓ moves, ↵ inserts into the app you came from,
 * ⇧↵ copies. Hides on ESC or blur.
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
      panelInsertSecret(snippet.id).catch((error: unknown) => {
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
            void panelInsertTemplate(snippet.id, {}).catch((error: unknown) => {
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
    panelInsert(snippet.id).catch((error: unknown) => {
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
    // In fill/verify mode the sub-view owns the keyboard (↵ acts, esc backs).
    if (filling !== null || verifying !== null) return;
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
    if (event.key === 'Enter') {
      event.preventDefault();
      const snippet = results[selected];
      if (snippet === undefined) return;
      if (event.shiftKey) copy(snippet);
      else insert(snippet);
    }
  };

  return (
    <div ref={rootRef} className="tv-panel" onKeyDown={onKeyDown}>
      {/* 28% scrim, no backdrop blur (motion spec); clicking it dismisses. */}
      <div
        key={`scrim-${showId}`}
        className="tv-panel-scrim"
        aria-hidden="true"
        data-testid="panel-scrim"
        onMouseDown={() => void hidePanel()}
      />
      <div key={showId} className="tv-panel-card tv-panel-enter">
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
            <div className="tv-panel-field">
              <SearchLine
                scale="panel"
                value={query}
                onChange={onQueryChange}
                label={tr('Search snippets', '搜索片段')}
                placeholder={tr('Search snippets…', '搜索片段…')}
                trailing={
                  destination !== null ? (
                    <span className="tv-panel-dest">→ {destination}</span>
                  ) : undefined
                }
              />
            </div>

            <div className="tv-panel-body" role="listbox" aria-label={tr('Snippets', '片段')}>
              {results.length === 0 ? (
                <p className="tv-panel-empty">
                  {query.trim() === ''
                    ? tr('No snippets yet', '还没有片段')
                    : tr('No matches', '没有匹配结果')}
                </p>
              ) : (
                results.map((snippet, index) => (
                  <div
                    key={snippet.id}
                    role="option"
                    aria-selected={index === selected}
                    data-selected={index === selected || undefined}
                    className="tv-panel-row"
                    onMouseEnter={() => setSelected(index)}
                    onClick={() => insert(snippet)}
                  >
                    <TypeMark code={markForType(snippet.snippetType)} />
                    <span className="tv-panel-row-main">
                      <span className="tv-panel-row-title">{snippet.title}</span>
                      <span className="tv-panel-row-preview" aria-hidden="true">
                        {previewLine(snippet.body)}
                      </span>
                    </span>
                    {snippet.trigger !== null && (
                      <span className="tv-panel-row-trigger">{snippet.trigger}</span>
                    )}
                    {index === selected && (
                      <span aria-hidden="true" className="tv-panel-row-enter">
                        ↵
                      </span>
                    )}
                  </div>
                ))
              )}
              {hiddenByRules > 0 && (
                <p className="tv-panel-hidden">
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
              <p className="tv-panel-notice" role="status">
                {ruleNotice}
              </p>
            )}

            <footer className="tv-panel-footer">
              <span className="tv-panel-keys">
                <kbd>↑</kbd>
                <kbd>↓</kbd>
                <span className="tv-panel-key-word">{tr('move', '移动')}</span>
                <kbd>↵</kbd>
                <span className="tv-panel-key-word">{tr('insert', '插入')}</span>
                <kbd>⇧↵</kbd>
                <span className="tv-panel-key-word">{tr('copy', '复制')}</span>
                <kbd>/tp</kbd>
                <span className="tv-panel-key-word">{tr('filter', '筛选')}</span>
                <kbd>esc</kbd>
                <span className="tv-panel-key-word">{tr('close', '关闭')}</span>
              </span>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}
