// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Typvia command palette (⌘K): navigate anywhere, or start something
 * new, without leaving the keyboard. Entries are provided by the shell so
 * this file stays free of routing and IPC.
 */
import { useTr } from '@typvia/ui';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Glyph } from './kit';

export interface PaletteEntry {
  id: string;
  group: string;
  label: string;
  /** Rendered right-aligned, e.g. "⌘1". */
  shortcut?: string;
  run: () => void;
}

interface PaletteProps {
  entries: readonly PaletteEntry[];
  onClose: () => void;
}

export function CommandPalette({ entries, onClose }: PaletteProps) {
  const tr = useTr();
  const [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => input.current?.focus(), []);

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle === '') return entries;
    return entries.filter((entry) =>
      `${entry.group} ${entry.label}`.toLowerCase().includes(needle),
    );
  }, [entries, query]);

  // Keep the cursor on an existing row as the list narrows.
  const active = matches.length === 0 ? -1 : Math.min(cursor, matches.length - 1);

  const run = (entry: PaletteEntry | undefined) => {
    if (entry === undefined) return;
    onClose();
    entry.run();
  };

  let lastGroup = '';
  return (
    <>
      <div aria-hidden="true" className="tvw-scrim-layer" onClick={onClose} />
      <div
        className="tvw-palette"
        role="dialog"
        aria-modal="true"
        aria-label={tr('Search Typvia', '搜索 Typvia')}
      >
        <div className="tvw-palette-field">
          <Glyph name="search" />
          <input
            ref={input}
            type="text"
            value={query}
            placeholder={tr('Search Typvia…', '搜索 Typvia…')}
            aria-label={tr('Search Typvia', '搜索 Typvia')}
            onChange={(event) => {
              setQuery(event.target.value);
              setCursor(0);
            }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setCursor((current) => Math.min(current + 1, matches.length - 1));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setCursor((current) => Math.max(current - 1, 0));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                run(matches[active]);
              }
            }}
          />
        </div>
        <div className="tvw-palette-list">
          {matches.length === 0 && (
            <p className="tvw-palette-empty">
              {tr('Nothing matches that yet.', '还没有匹配的内容。')}
            </p>
          )}
          {matches.map((entry, index) => {
            const heading = entry.group === lastGroup ? null : entry.group;
            lastGroup = entry.group;
            return (
              <div key={entry.id}>
                {heading !== null && <div className="tvw-pop-label">{heading}</div>}
                <button
                  type="button"
                  className="tvw-palette-item"
                  aria-selected={index === active}
                  onMouseEnter={() => setCursor(index)}
                  onClick={() => run(entry)}
                >
                  {entry.label}
                  {entry.shortcut !== undefined && (
                    <span aria-hidden="true" className="tvw-nav-shortcut">
                      {entry.shortcut}
                    </span>
                  )}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
