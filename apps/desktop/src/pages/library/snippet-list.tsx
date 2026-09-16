// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { ListOrder, Snippet } from '@typvia/shared';
import { useTr, useVirtualRows } from '@typvia/ui';
import { useEffect, useRef, useState, type MouseEvent, type PointerEvent } from 'react';
import { Choice } from '../../paper/choice';
import { KeyCap, TextAction } from '../../paper/kit';
import { useVariables } from './library-data';
import { TRIGGER_MIN_CHARS, triggerChars } from './library-model';
import { SnippetRow, type RowFacts } from './snippet-row';
import type { SnippetPages } from './use-snippet-pages';

/** Fixed row height the windowing maths depends on; keep it in step with
    `.tvl-row` (padding, the content line, the meta line) in library.css. */
export const ROW_HEIGHT = 62;

export const rowId = (index: number) => `tvl-row-${String(index)}`;

/** Title, count, "New snippet" and — while browsing — the three orders. */
export function ListHead({
  title,
  count,
  order,
  onOrder,
  onNew,
}: {
  title: string;
  count: string;
  order: ListOrder | null;
  onOrder: (order: ListOrder) => void;
  onNew: () => void;
}) {
  const tr = useTr();
  return (
    <div className="tvl-head">
      <div className="tvl-head-line">
        <h1>{title}</h1>
        <span className="tvl-head-count">{count}</span>
        <span className="tvl-grow" />
        <TextAction onClick={onNew}>
          {tr('New snippet', '新建片段')}
          <KeyCap>⌘⇧N</KeyCap>
        </TextAction>
      </div>
      {order !== null && (
        <Choice
          label={tr('Order', '排序')}
          value={order}
          onChange={onOrder}
          options={[
            { value: 'recent', label: tr('Recently used', '最近用过') },
            { value: 'added', label: tr('Recently added', '最近添加') },
            { value: 'used', label: tr('Most used', '用得最多') },
          ]}
        />
      )}
    </div>
  );
}

/**
 * The collection's snippets as a virtual list: only the rows in view exist,
 * and only the pages touching them are fetched, so fifty thousand stay light.
 */
export function SnippetList({
  label,
  pages,
  pick,
  onPick,
  onRowMenu,
  onRowPress,
  factsFor,
}: {
  label: string;
  pages: SnippetPages;
  pick: number;
  onPick: (index: number) => void;
  onRowMenu: (event: MouseEvent, index: number, snippet: Snippet, hasVariables: boolean) => void;
  onRowPress: (event: PointerEvent, snippet: Snippet) => void;
  factsFor: (snippet: Snippet) => Omit<RowFacts, 'variables'>;
}) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const { first, last, totalHeight } = useVirtualRows({
    count: pages.total,
    rowHeight: ROW_HEIGHT,
    viewportRef,
  });
  const { ensureRange } = pages;
  useEffect(() => {
    ensureRange(first, last);
  }, [ensureRange, first, last]);

  // Keyboard picks walk past the edge of the view; the view follows.
  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport === null) return;
    const top = pick * ROW_HEIGHT;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + ROW_HEIGHT > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = top + ROW_HEIGHT - viewport.clientHeight;
    }
  }, [pick]);

  const visible: Array<{ index: number; snippet: Snippet }> = [];
  for (let index = first; index <= last && index < pages.total; index += 1) {
    const snippet = pages.rowAt(index);
    if (snippet !== undefined) visible.push({ index, snippet });
  }
  const variablesOf = useVariables(visible.map((row) => row.snippet));

  // The column widens to the longest trigger scrolled past and stays there, so
  // the text does not shift left and right while scrolling one list.
  const [widest, setWidest] = useState({ label, chars: TRIGGER_MIN_CHARS });
  const seen = widest.label === label ? widest.chars : TRIGGER_MIN_CHARS;
  const triggerWidth = Math.max(seen, triggerChars(visible.map((row) => row.snippet.trigger)));
  useEffect(() => {
    if (widest.label !== label || widest.chars !== triggerWidth) {
      setWidest({ label, chars: triggerWidth });
    }
  }, [label, triggerWidth, widest]);

  return (
    <div
      ref={viewportRef}
      className="tvl-viewport"
      role="listbox"
      tabIndex={0}
      aria-label={label}
      aria-activedescendant={pages.rowAt(pick) === undefined ? undefined : rowId(pick)}
      data-testid="library-viewport"
    >
      <div className="tvl-spacer" style={{ height: totalHeight }}>
        {visible.map(({ index, snippet }) => (
          <SnippetRow
            key={snippet.id}
            id={rowId(index)}
            snippet={snippet}
            facts={{ ...factsFor(snippet), variables: variablesOf(snippet)?.length ?? 0 }}
            picked={index === pick}
            triggerWidth={triggerWidth}
            style={{ transform: `translateY(${String(index * ROW_HEIGHT)}px)` }}
            onPick={() => onPick(index)}
            onMenu={(event) =>
              onRowMenu(event, index, snippet, (variablesOf(snippet)?.length ?? 0) > 0)
            }
            onPress={(event) => onRowPress(event, snippet)}
          />
        ))}
      </div>
    </div>
  );
}
