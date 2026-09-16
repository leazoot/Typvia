// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { templateVariables, type Snippet } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { KeyCap } from '../../paper/kit';
import { whenLabel } from '../library/when-label';

/** First non-empty line of the body. Sensitive snippets carry no body across IPC. */
export function previewLine(body: string | null): string {
  if (body === null) return '';
  return (
    body
      .split('\n')
      .map((line) => line.trim())
      .find((line) => line !== '') ?? ''
  );
}

/**
 * One Quick Bar row: the words themselves in the serif, then trigger, name and
 * when it was last used; a template says how many variables wait to be filled.
 */
export function PanelRow({
  snippet,
  selected,
  shortcut,
  onHover,
  onPick,
}: {
  snippet: Snippet;
  selected: boolean;
  /** The ⌘-digit that inserts this row, for the first nine. */
  shortcut: string | null;
  onHover: () => void;
  onPick: () => void;
}) {
  const tr = useTr();
  const locale = useLocale();
  const [variables, setVariables] = useState(0);
  const isTemplate = snippet.snippetType === 'template';
  const sensitive = snippet.securityLevel === 'sensitive';

  // Which names count as variables is the template engine's call, not the panel's.
  useEffect(() => {
    if (!isTemplate || snippet.body === null) return undefined;
    let live = true;
    templateVariables(snippet.body)
      .then((names) => {
        if (live) setVariables(names.length);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [isTemplate, snippet.body]);

  const line = sensitive ? '' : previewLine(snippet.body);

  return (
    <div
      role="option"
      aria-selected={selected}
      data-selected={selected || undefined}
      className="tvq-row"
      onMouseEnter={onHover}
      onClick={onPick}
    >
      <div className="tvq-row-main">
        <div className="tvq-row-words" aria-hidden={line !== '' || undefined}>
          {sensitive ? '••••••••••' : line === '' ? snippet.title : line}
        </div>
        <div className="tvq-row-meta">
          {snippet.trigger !== null && <span className="tvq-mono">{snippet.trigger}</span>}
          {(sensitive || (line !== '' && !line.startsWith(snippet.title))) && (
            <span>{snippet.title}</span>
          )}
          {sensitive && <span>{tr('In the vault', '在保险库里')}</span>}
          {snippet.lastUsedAt !== null && (
            <span>{whenLabel(snippet.lastUsedAt, Date.now(), locale)}</span>
          )}
          {variables > 0 && (
            <span className="tvq-flag">
              <span aria-hidden="true" className="tvq-flag-dot" />
              {tr(
                `${String(variables)} variable${variables === 1 ? '' : 's'} to fill`,
                `${String(variables)} 个变量待填`,
              )}
            </span>
          )}
        </div>
      </div>
      {shortcut !== null && <KeyCap>{shortcut}</KeyCap>}
    </div>
  );
}
