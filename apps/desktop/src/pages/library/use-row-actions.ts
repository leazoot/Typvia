// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  batchMoveSnippets,
  copySnippet,
  mainInsert,
  mainInsertTemplate,
  restoreSnippet,
  snippetConvertToSensitive,
  toIpcError,
  trashSnippet,
} from '@typvia/shared';
import type { Snippet } from '@typvia/shared';
import { useTr, type Tr } from '@typvia/ui';
import { useState } from 'react';
import { useEspanso } from '../../espanso/espanso-context';
import { loadInsertMethod } from '../../workspace/insert-method';
import { announceLibraryChange } from '../../workspace/library-events';
import { useUndo } from '../../workspace/undo';
import type { InsertFailure } from './snippet-detail';

/** How a note names a snippet: by its trigger, or by its title in quotes. */
export function nameOf(snippet: Snippet, tr: Tr): string {
  if (snippet.trigger !== null) return snippet.trigger;
  return tr(`“${snippet.title}”`, `「${snippet.title}」`);
}

/**
 * What can be done to one snippet from the main window. Nothing here asks
 * first: a delete happens and the note offers it back; a failure says what
 * is still as it was.
 */
export function useRowActions(onUsed: () => void) {
  const tr = useTr();
  const { offer, say } = useUndo();
  const { notifyMutation } = useEspanso();
  const [failure, setFailure] = useState<{ id: string; kind: InsertFailure } | null>(null);

  // Triggers leave or come back with the snippet, so the expansion config follows.
  const changed = () => {
    notifyMutation();
    announceLibraryChange();
  };

  const delivered = (snippet: Snippet, sent: Promise<void>) => {
    setFailure(null);
    sent.then(onUsed).catch((error: unknown) => {
      const code = toIpcError(error).code;
      const kind: InsertFailure =
        code === 'permission_denied' ? 'permission' : code === 'rule_blocked' ? 'rule' : 'other';
      setFailure({ id: snippet.id, kind });
    });
  };

  const insert = (snippet: Snippet) =>
    delivered(snippet, mainInsert(snippet.id, loadInsertMethod()));

  const insertTemplate = (snippet: Snippet, values: Record<string, string>) =>
    delivered(snippet, mainInsertTemplate(snippet.id, values, loadInsertMethod()));

  const copy = (snippet: Snippet) => {
    copySnippet(snippet.id)
      .then(() => {
        setFailure(null);
        onUsed();
      })
      .catch(() => setFailure({ id: snippet.id, kind: 'other' }));
  };

  const remove = (snippet: Snippet) => {
    const name = nameOf(snippet, tr);
    trashSnippet(snippet.id)
      .then(() => {
        changed();
        offer({
          title: tr(`Deleted ${name}.`, `已删掉 ${name}。`),
          body: tr(
            '⌘Z takes it back, or find it in the trash within 30 days.',
            '⌘Z 撤回,或者 30 天内在回收站里找回。',
          ),
          failure: tr('It is still in the trash for 30 days.', '它还在回收站里,30 天内都能找回。'),
          undo: () => restoreSnippet(snippet.id).then(changed),
        });
      })
      .catch(() =>
        say({
          title: tr(`${name} wasn't deleted.`, `${name} 没删掉。`),
          body: tr('Nothing changed; you can try again.', '什么都没变,可以再试一次。'),
        }),
      );
  };

  const moveTo = (snippet: Snippet, folderId: string) => {
    batchMoveSnippets([snippet.id], folderId === '' ? null : folderId)
      .then(announceLibraryChange)
      .catch(() =>
        say({
          title: tr(`${nameOf(snippet, tr)} didn't move.`, `${nameOf(snippet, tr)} 没移过去。`),
          body: tr('It is still in the collection it was in.', '它还在原来的集合里。'),
        }),
      );
  };

  const toVault = (snippet: Snippet) => {
    const name = nameOf(snippet, tr);
    snippetConvertToSensitive(snippet.id)
      .then(() => {
        changed();
        say({
          mood: 'happy',
          title: tr(`${name} is in the vault.`, `${name} 收进保险库了。`),
          body: tr(
            'It is encrypted now, and no longer in the library or in search.',
            '已经加密,不再出现在资料库和搜索里。',
          ),
        });
      })
      .catch(() =>
        say({
          title: tr(`${name} didn't go in.`, `${name} 没收进去。`),
          body: tr('It is unchanged and still in the library.', '片段没有变,还在资料库里。'),
        }),
      );
  };

  return { failure, insert, insertTemplate, copy, remove, moveTo, toVault };
}
