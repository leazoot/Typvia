// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Snippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import type { CSSProperties, MouseEvent, PointerEvent, ReactNode } from 'react';
import { firstLine, isCode } from './library-model';

/** Marks every occurrence of the query with a wash — never an underline. */
export function Highlight({ text, query }: { text: string; query: string }) {
  const needle = query.trim().toLowerCase();
  if (needle === '') return <>{text}</>;
  const haystack = text.toLowerCase();
  const parts: ReactNode[] = [];
  let from = 0;
  for (let at = haystack.indexOf(needle); at !== -1; at = haystack.indexOf(needle, from)) {
    parts.push(
      text.slice(from, at),
      <mark key={at} className="tvl-mark">
        {text.slice(at, at + needle.length)}
      </mark>,
    );
    from = at + needle.length;
  }
  parts.push(text.slice(from));
  return <>{parts}</>;
}

/** What a row says under its text, worked out by the page that knows the collections. */
export interface RowFacts {
  collection: string;
  used: string;
  variables: number;
}

/**
 * One snippet line: trigger cap, the body's first line in the content face,
 * and a quiet meta line. No divider between rows — the picked one rises onto
 * paper-0, a hovered one sinks into paper-2.
 */
export function SnippetRow({
  id,
  snippet,
  facts,
  picked,
  query = '',
  triggerWidth,
  style,
  onPick,
  onMenu,
  onPress,
}: {
  id: string;
  snippet: Snippet;
  facts: RowFacts;
  picked: boolean;
  query?: string;
  /** Characters the trigger column holds, shared by every row in the list. */
  triggerWidth: number;
  style?: CSSProperties;
  onPick: () => void;
  onMenu: (event: MouseEvent) => void;
  /** A press that may become a drag onto a collection. */
  onPress: (event: PointerEvent) => void;
}) {
  const tr = useTr();
  const variables = String(facts.variables);
  return (
    <div
      id={id}
      role="option"
      aria-selected={picked}
      className="tvl-row"
      style={style}
      onClick={onPick}
      onContextMenu={onMenu}
      onPointerDown={onPress}
    >
      {/* The cap is border-box: its padding and hairline sit inside the width. */}
      <span className="tvl-trigger" style={{ width: `calc(${String(triggerWidth)}ch + 14px)` }}>
        {snippet.trigger === null ? '—' : <Highlight text={snippet.trigger} query={query} />}
      </span>
      <span className="tvl-row-main">
        <span className={isCode(snippet) ? 'tvl-row-body is-code' : 'tvl-row-body'}>
          <Highlight text={firstLine(snippet)} query={query} />
        </span>
        <span className="tvl-row-meta">
          <span>{`${facts.collection} · ${facts.used}`}</span>
          {facts.variables > 0 && (
            <span className="tvl-row-vars">
              <span aria-hidden="true" className="tvl-row-vars-dot" />
              {tr(
                facts.variables === 1 ? '1 variable' : `${variables} variables`,
                `${variables} 个变量`,
              )}
            </span>
          )}
        </span>
      </span>
    </div>
  );
}
