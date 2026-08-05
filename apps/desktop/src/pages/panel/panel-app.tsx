import { listen } from '@tauri-apps/api/event';
import {
  copySnippet,
  hidePanel,
  IpcError,
  listSnippetPage,
  panelInsert,
  panelReady,
  searchLibrary,
} from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { SearchLine, TypeMark } from '@typvia/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import { markFor } from '../library/preview';
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
 * Global command panel (design Phase 1 · 1e/1f/1g, wired Phase 9). The search
 * field is focused at frame 0 — you type through the enter animation. Summoned
 * by ⌘⇧V; the empty query shows recent snippets, typing replaces them with
 * ranked results (no debounce). ↑↓ moves, ↵ inserts into the app you came from,
 * ⇧↵ copies. Hides on ESC or blur.
 */
export function PanelApp() {
  const [query, setQuery] = useState('');
  // Bumped on every summon so the enter animation replays and the field
  // re-focuses (the resident WebView is never torn down between shows).
  const [showId, setShowId] = useState(0);
  const [destination, setDestination] = useState<string | null>(null);
  const [results, setResults] = useState<Snippet[]>([]);
  const [selected, setSelected] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  // Guards against out-of-order responses: only the latest load may apply.
  const loadSeq = useRef(0);

  const load = useCallback((q: string) => {
    const seq = (loadSeq.current += 1);
    const request =
      q.trim() === ''
        ? listSnippetPage('recent', null, null, RESULT_LIMIT, 0)
        : searchLibrary(q, RESULT_LIMIT);
    request
      .then((rows) => {
        if (loadSeq.current === seq) {
          setResults(rows);
          setSelected(0);
        }
      })
      .catch(() => {
        if (loadSeq.current === seq) {
          setResults([]);
          setSelected(0);
        }
      });
  }, []);

  useEffect(() => {
    const pending = listen<{ destination: string | null }>('panel:show', (event) => {
      setDestination(event.payload.destination);
      setQuery('');
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
    load(value);
  };

  const insert = (snippet: Snippet) => {
    // The host hides the panel and restores focus before injecting; a denied
    // permission falls back to copying so the action is never silently lost.
    panelInsert(snippet.id).catch((error: unknown) => {
      if (error instanceof IpcError && error.code === 'permission_denied') {
        void copySnippet(snippet.id);
      }
    });
  };

  const copy = (snippet: Snippet) => {
    void copySnippet(snippet.id).finally(() => {
      void hidePanel();
    });
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
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
      <div key={showId} className="tv-panel-enter">
        <div className="tv-panel-field">
          <SearchLine
            scale="panel"
            value={query}
            onChange={onQueryChange}
            label="Search snippets"
            placeholder="Search snippets…"
            trailing={
              destination !== null ? (
                <span className="tv-panel-dest">→ {destination}</span>
              ) : undefined
            }
          />
        </div>

        <div className="tv-panel-body" role="listbox" aria-label="Snippets">
          {results.length === 0 ? (
            <p className="tv-panel-empty">
              {query.trim() === '' ? 'No snippets yet' : 'No matches'}
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
                <TypeMark code={markFor(snippet.snippetType)} />
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
        </div>

        <footer className="tv-panel-footer">
          <span className="tv-panel-keys">
            <kbd>↑</kbd>
            <kbd>↓</kbd>
            <span className="tv-panel-key-word">move</span>
            <kbd>↵</kbd>
            <span className="tv-panel-key-word">insert</span>
            <kbd>⇧↵</kbd>
            <span className="tv-panel-key-word">copy</span>
            <kbd>esc</kbd>
            <span className="tv-panel-key-word">close</span>
          </span>
        </footer>
      </div>
    </div>
  );
}
