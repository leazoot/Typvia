// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  batchMoveSnippets,
  createFolder,
  deleteFolder,
  listSnippetPage,
  mergeFolders,
  reorderFolders,
  updateFolder,
} from '@typvia/shared';
import type { Folder } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { KeyCap, Note } from '../../paper/kit';
import { announceLibraryChange } from '../../workspace/library-events';
import { useUndo } from '../../workspace/undo';
import { usePointerDrag } from './use-pointer-drag';

/** The host's largest page and batch. */
const BATCH = 500;

async function idsInFolder(folderId: string): Promise<string[]> {
  const ids: string[] = [];
  for (let offset = 0; ; offset += BATCH) {
    const page = await listSnippetPage('folder', folderId, null, BATCH, offset);
    ids.push(...page.map((snippet) => snippet.id));
    if (page.length < BATCH) return ids;
  }
}

/** Puts a collection that is gone back under its old name, with its snippets in it. */
async function recreate(folder: Folder, ids: readonly string[]): Promise<void> {
  const made = await createFolder({
    name: folder.name,
    parentId: folder.parentId,
    sortOrder: folder.sortOrder,
  });
  for (let at = 0; at < ids.length; at += BATCH) {
    await batchMoveSnippets(ids.slice(at, at + BATCH), made.id);
  }
  announceLibraryChange();
}

/** A name typed in place: one line under the text, ⏎ keeps it, esc leaves it. */
function NameLine({
  initial,
  label,
  hint,
  onCommit,
  onCancel,
}: {
  initial: string;
  label: string;
  hint: string;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  const input = useRef<HTMLInputElement>(null);
  const settled = useRef(false);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  const settle = (keep: boolean) => {
    if (settled.current) return;
    settled.current = true;
    if (keep) onCommit(value);
    else onCancel();
  };

  return (
    <div className="tvl-manage-edit">
      <div className="tvl-name-line">
        <input
          ref={input}
          type="text"
          value={value}
          aria-label={label}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              settle(true);
            } else if (event.key === 'Escape') {
              event.preventDefault();
              settle(false);
            }
          }}
          onBlur={() => settle(true)}
        />
      </div>
      <p className="tvl-manage-hint">{hint}</p>
    </div>
  );
}

function Handle() {
  return (
    <svg width="8" height="12" viewBox="0 0 8 12" fill="none" aria-hidden="true">
      <path
        d="M1 2.5h6M1 6h6M1 9.5h6"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
    </svg>
  );
}

/**
 * The collections column while it is being arranged: no menu and no dialog.
 * A name is changed where it stands; dragging a collection between two others
 * moves it, dropping it onto another merges the two after one question on a
 * note; deleting happens at once and the undo note brings it back.
 */
export function CollectionsManage({
  folders,
  countOf,
  unsortedCount,
  creating,
  onCreate,
  onCreateEnd,
}: {
  folders: readonly Folder[];
  countOf: (folderId: string) => number;
  unsortedCount: number;
  creating: boolean;
  onCreate: () => void;
  onCreateEnd: () => void;
}) {
  const tr = useTr();
  const { offer, say } = useUndo();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [merge, setMerge] = useState<{ source: Folder; target: Folder } | null>(null);
  const count = (n: number) => tr(String(n), `${String(n)} 条`);
  const failed = (title: string) =>
    say({ title, body: tr('Your collections are as they were.', '集合都还是原样。') });

  const rename = (folder: Folder, name: string) => {
    setRenaming(null);
    if (name.trim() === '' || name.trim() === folder.name) return;
    updateFolder({ id: folder.id, name, parentId: folder.parentId, sortOrder: folder.sortOrder })
      .then(announceLibraryChange)
      .catch(() => failed(tr(`“${folder.name}” kept its name.`, `「${folder.name}」没改成名字。`)));
  };

  const create = (name: string) => {
    onCreateEnd();
    if (name.trim() === '') return;
    createFolder({ name, parentId: null, sortOrder: folders.length })
      .then(announceLibraryChange)
      .catch(() => failed(tr("The collection wasn't made.", '集合没建出来。')));
  };

  const remove = async (folder: Folder) => {
    try {
      const ids = await idsInFolder(folder.id);
      await deleteFolder(folder.id);
      announceLibraryChange();
      const n = String(ids.length);
      offer({
        title: tr(`Deleted the “${folder.name}” collection.`, `已删掉集合「${folder.name}」。`),
        body: tr(
          `Its ${n} snippets are in Unsorted now. ⌘Z takes it back.`,
          `里面 ${n} 条回到了「待整理」。⌘Z 撤回。`,
        ),
        failure: tr(
          'Its snippets are all safe in Unsorted.',
          '里面的片段都在「待整理」里,一条没丢。',
        ),
        undo: () => recreate(folder, ids),
      });
    } catch {
      failed(tr(`“${folder.name}” wasn't deleted.`, `「${folder.name}」没删掉。`));
    }
  };

  const runMerge = async ({ source, target }: { source: Folder; target: Folder }) => {
    setMerge(null);
    try {
      const moved = await mergeFolders(source.id, target.id);
      announceLibraryChange();
      offer({
        title: tr(
          `Merged “${source.name}” into “${target.name}”.`,
          `已把「${source.name}」并进「${target.name}」。`,
        ),
        body: tr('Every trigger is unchanged. ⌘Z takes it back.', '触发词一个都没变。⌘Z 撤回。'),
        failure: tr(
          `The snippets are all in “${target.name}”.`,
          `片段都在「${target.name}」里,一条没丢。`,
        ),
        undo: () => recreate(source, moved),
      });
    } catch {
      failed(tr("The two collections weren't merged.", '两个集合没并起来。'));
    }
  };

  const reorder = (folder: Folder, target: Folder, after: boolean) => {
    const ids = folders.map((f) => f.id).filter((id) => id !== folder.id);
    ids.splice(ids.indexOf(target.id) + (after ? 1 : 0), 0, folder.id);
    reorderFolders(ids)
      .then(announceLibraryChange)
      .catch(() => failed(tr("The order didn't change.", '顺序没改过来。')));
  };

  const drag = usePointerDrag<Folder>({
    targets: '.tvl-manage-row[data-value]',
    zones: true,
    onDrop: (folder, spot) => {
      const target = folders.find((f) => f.id === spot.value);
      if (target === undefined || target.id === folder.id) return;
      if (spot.zone === 'on') setMerge({ source: folder, target });
      else reorder(folder, target, spot.zone === 'after');
    },
  });

  // Alt + ↑ / ↓ moves a collection for anyone not using a pointer.
  const nudge = (event: KeyboardEvent, index: number) => {
    if (!event.altKey || (event.key !== 'ArrowUp' && event.key !== 'ArrowDown')) return;
    event.preventDefault();
    const folder = folders[index];
    const target = folders[index + (event.key === 'ArrowUp' ? -1 : 1)];
    if (folder !== undefined && target !== undefined) {
      reorder(folder, target, event.key === 'ArrowDown');
    }
  };

  return (
    <div
      className="tvl-manage"
      onKeyDown={(event) => {
        // Keys typed here are for these rows, not for the snippet list.
        if (!event.metaKey && !event.ctrlKey) event.stopPropagation();
      }}
    >
      <div className="tvl-manage-rows">
        {folders.map((folder, index) => {
          if (renaming === folder.id) {
            const n = String(countOf(folder.id));
            return (
              <NameLine
                key={folder.id}
                initial={folder.name}
                label={tr(`New name for ${folder.name}`, `「${folder.name}」的新名字`)}
                hint={tr(
                  `⏎ to keep it · esc to leave it. The triggers of its ${n} snippets stay as they are.`,
                  `⏎ 改好 · esc 不改。${n} 条片段的触发词一个都不会变。`,
                )}
                onCommit={(name) => rename(folder, name)}
                onCancel={() => setRenaming(null)}
              />
            );
          }
          const spot =
            drag.drag !== null &&
            drag.drag.item.id !== folder.id &&
            drag.drag.spot?.value === folder.id
              ? drag.drag.spot.zone
              : undefined;
          return (
            <div
              key={folder.id}
              className="tvl-manage-row"
              data-value={folder.id}
              data-drop={spot}
              data-dragging={drag.drag?.item.id === folder.id ? true : undefined}
            >
              <span className="tvl-handle" onPointerDown={(event) => drag.start(event, folder)}>
                <Handle />
              </span>
              <button
                type="button"
                className="tvl-manage-name"
                aria-label={tr(`Rename ${folder.name}`, `给「${folder.name}」改名`)}
                onClick={() => setRenaming(folder.id)}
                onKeyDown={(event) => nudge(event, index)}
              >
                {folder.name}
              </button>
              {spot === 'on' && (
                <span className="tvl-drop-note">{tr('Merge here', '并到这里')}</span>
              )}
              <span className="tvl-manage-count">{count(countOf(folder.id))}</span>
              <button
                type="button"
                className="tvl-manage-delete"
                aria-label={tr(`Delete ${folder.name}`, `删掉「${folder.name}」`)}
                onClick={() => void remove(folder)}
              >
                {tr('Delete', '删')}
              </button>
            </div>
          );
        })}
        <div className="tvl-manage-row is-system">
          <span className="tvl-handle is-blank" aria-hidden="true" />
          <span className="tvl-manage-name">{tr('Unsorted', '待整理')}</span>
          <span className="tvl-manage-note">
            {tr('Built in · cannot be deleted', '系统自带 · 不能删')}
          </span>
          <span className="tvl-manage-count">{count(unsortedCount)}</span>
        </div>
        {creating && (
          <NameLine
            initial=""
            label={tr('Name of the new collection', '新集合的名字')}
            hint={tr('⏎ to make it · esc to leave it.', '⏎ 建好 · esc 不建。')}
            onCommit={create}
            onCancel={onCreateEnd}
          />
        )}
      </div>

      {merge !== null && (
        <div className="tvl-merge">
          <Note
            label={tr(
              `Merge “${merge.source.name}” into “${merge.target.name}”?`,
              `把「${merge.source.name}」并进「${merge.target.name}」?`,
            )}
          >
            <p className="tpi-note-title">
              {tr(
                `Merge “${merge.source.name}” into “${merge.target.name}”?`,
                `把「${merge.source.name}」并进「${merge.target.name}」?`,
              )}
            </p>
            <p className="tpi-note-body">
              {tr(
                `${String(countOf(merge.source.id))} snippets move over and every trigger stays. “${merge.source.name}” is gone afterwards — ⌘Z brings it back.`,
                `${String(countOf(merge.source.id))} 条会搬过去,触发词一个都不变。并完「${merge.source.name}」就没了 —— 这一步可以用 ⌘Z 撤回。`,
              )}
            </p>
            <div className="tpi-note-actions">
              <button type="button" className="tpi-note-yes" onClick={() => void runMerge(merge)}>
                {tr('Merge', '并过去')}
              </button>
              <button type="button" className="tpi-note-no" onClick={() => setMerge(null)}>
                {tr('Not now', '先别')}
              </button>
            </div>
          </Note>
        </div>
      )}

      <div className="tvl-manage-rule" />
      <button type="button" className="tpi-action tvl-manage-new" onClick={onCreate}>
        {tr('New collection', '新建集合')}
        <KeyCap>⌘⇧K</KeyCap>
      </button>
    </div>
  );
}
