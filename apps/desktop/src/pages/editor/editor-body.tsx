// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';

/** Splits body text into plain runs and `{{variable}}` token runs. */
export function splitTokens(text: string): Array<{ token: boolean; text: string }> {
  return text
    .split(/(\{\{[^{}]+\}\})/g)
    .filter((part) => part !== '')
    .map((part) => ({ token: /^\{\{[^{}]+\}\}$/.test(part), text: part }));
}

/** Unique variable names inside `{{…}}` markers, in order of appearance. */
export function variableNames(text: string): string[] {
  const names: string[] = [];
  for (const match of text.matchAll(/\{\{\s*([^{}]+?)\s*\}\}/g)) {
    const name = match[1];
    if (name !== undefined && !names.includes(name)) names.push(name);
  }
  return names;
}

interface EditorBodyProps {
  value: string;
  onChange: (value: string) => void;
}

/** Maps a click inside the styled view to a character offset in the source. */
function offsetFromPoint(container: HTMLElement, x: number, y: number): number | null {
  if (typeof document.caretRangeFromPoint !== 'function') return null;
  const range = document.caretRangeFromPoint(x, y);
  if (range === null || !container.contains(range.startContainer)) return null;
  let offset = range.startOffset;
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node !== null && node !== range.startContainer) {
    offset += node.textContent?.length ?? 0;
    node = walker.nextNode();
  }
  return node === range.startContainer ? offset : null;
}

/**
 * The body editor, in the content face at reading size. At rest the
 * text renders styled — `{{variable}}` runs become inline tokens whose
 * underline reads as a fill-in blank. Interacting swaps in a textarea over
 * the same text; the styled view's text content is the source verbatim, so
 * the clicked caret position carries over 1:1.
 */
export function EditorBody({ value, onChange }: EditorBodyProps) {
  const tr = useTr();
  const [editing, setEditing] = useState(value === '');
  const [caret, setCaret] = useState<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!editing || textarea === null) return;
    textarea.focus();
    const at = caret ?? textarea.value.length;
    textarea.setSelectionRange(at, at);
    textarea.style.height = 'auto';
    textarea.style.height = `${String(textarea.scrollHeight)}px`;
  }, [editing, caret]);

  if (!editing) {
    return (
      <div
        className="tve-body is-view"
        role="button"
        tabIndex={0}
        aria-label={tr('Edit snippet body', '编辑片段正文')}
        onMouseDown={(event) => {
          event.preventDefault();
          setCaret(offsetFromPoint(event.currentTarget, event.clientX, event.clientY));
          setEditing(true);
        }}
        onFocus={() => {
          setCaret(null);
          setEditing(true);
        }}
      >
        {splitTokens(value).map((part, index) =>
          part.token ? (
            <span key={index} className="tve-token">
              {part.text}
            </span>
          ) : (
            <span key={index}>{part.text}</span>
          ),
        )}
      </div>
    );
  }

  return (
    <textarea
      ref={textareaRef}
      className="tve-body is-input"
      aria-label={tr('Snippet body', '片段正文')}
      placeholder={tr('Write the snippet body…', '在此撰写片段正文…')}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
        event.target.style.height = 'auto';
        event.target.style.height = `${String(event.target.scrollHeight)}px`;
      }}
      onBlur={() => {
        if (value !== '') setEditing(false);
      }}
    />
  );
}
