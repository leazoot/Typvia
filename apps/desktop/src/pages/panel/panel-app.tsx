import { listen } from '@tauri-apps/api/event';
import { hidePanel, panelReady } from '@typvia/shared';
import { SearchLine } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import './panel.css';

/** Window label of the resident panel (must match tauri.conf.json). */
export const PANEL_WINDOW_LABEL = 'panel';

/** Focuses the search input; the resident WebView must re-focus on every show. */
function focusSearch(root: HTMLElement | null): void {
  root?.querySelector('input')?.focus();
}

/**
 * Global command panel (design Phase 1 · 1e/1f/1g, wired Phase 9). The search
 * field is focused at frame 0 — the single most important detail in the
 * product: you type through the enter animation. Summoned by ⌘⇧V; hides on ESC
 * or blur, returning focus to the app you came from.
 *
 * This screen owns the window shell and the frame-0 gesture. Live search,
 * the result list, preview and injection are wired in TASK-039.
 */
export function PanelApp() {
  const [query, setQuery] = useState('');
  // Bumped on every summon so the enter animation replays and the field
  // re-focuses (the resident WebView is never torn down between shows).
  const [showId, setShowId] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const pending = listen('panel:show', () => {
      setQuery('');
      setShowId((id) => id + 1);
    });
    return () => {
      void pending.then((unlisten) => {
        unlisten();
      });
    };
  }, []);

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

  return (
    <div
      ref={rootRef}
      className="tv-panel"
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          void hidePanel();
        }
      }}
    >
      <div key={showId} className="tv-panel-enter">
        <div className="tv-panel-field">
          <SearchLine
            scale="panel"
            value={query}
            onChange={setQuery}
            label="Search snippets"
            placeholder="Search snippets…"
          />
        </div>
        <div className="tv-panel-body" />
        <footer className="tv-panel-footer">
          <span className="tv-panel-keys">
            <kbd>↑</kbd>
            <kbd>↓</kbd>
            <span className="tv-panel-key-word">move</span>
            <kbd>↵</kbd>
            <span className="tv-panel-key-word">insert</span>
            <kbd>esc</kbd>
            <span className="tv-panel-key-word">close</span>
          </span>
        </footer>
      </div>
    </div>
  );
}
