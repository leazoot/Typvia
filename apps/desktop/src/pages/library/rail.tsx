// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Folder, LibraryCounts, Tag } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useState } from 'react';
import type { LibraryScope } from './use-snippet-pages';

/** A folder flattened out of the tree with its indent depth. */
export interface FolderEntry {
  folder: Folder;
  depth: number;
}

type CountKey = 'total' | 'recent' | 'starred' | 'unsorted';

const VIEWS: Array<{ view: LibraryScope['view']; en: string; zh: string; countKey: CountKey }> = [
  { view: 'all', en: 'All snippets', zh: '全部片段', countKey: 'total' },
  { view: 'recent', en: 'Recent', zh: '最近', countKey: 'recent' },
  { view: 'starred', en: 'Starred', zh: '星标', countKey: 'starred' },
  { view: 'unsorted', en: 'Unsorted', zh: '未分类', countKey: 'unsorted' },
];

/** Which management interaction is open; the rail shows one at a time. */
type RailMode =
  | { kind: 'idle' }
  | { kind: 'new-folder'; parentId: string | null }
  | { kind: 'rename-folder'; id: string }
  | { kind: 'confirm-delete-folder'; id: string }
  | { kind: 'new-tag' }
  | { kind: 'rename-tag'; id: string }
  | { kind: 'confirm-delete-tag'; id: string };

interface RailProps {
  counts: LibraryCounts | null;
  folders: FolderEntry[];
  tags: Tag[];
  scope: LibraryScope;
  onScopeChange: (scope: LibraryScope) => void;
  onCreateFolder: (name: string, parentId: string | null) => void;
  onRenameFolder: (folder: Folder, name: string) => void;
  onDeleteFolder: (folder: Folder) => void;
  /** ⌥↑ / ⌥↓ moves a folder among its siblings. */
  onReorderFolder: (folder: Folder, direction: -1 | 1) => void;
  /** Live snippet count of a folder's whole subtree (delete-confirm copy). */
  subtreeCount: (folderId: string) => number;
  onCreateTag: (name: string) => void;
  onRenameTag: (tag: Tag, name: string) => void;
  onDeleteTag: (tag: Tag) => void;
}

function formatCount(count: number): string {
  return count.toLocaleString('en-US');
}

/** Chip-styled inline name input; Enter commits, Escape cancels. */
function NameInput({
  label,
  defaultValue,
  onCommit,
  onCancel,
}: {
  label: string;
  defaultValue?: string | undefined;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(defaultValue ?? '');
  return (
    <input
      // Transient management input: grabbing focus is the point.
      autoFocus
      type="text"
      className="tv-lib-rail-input"
      aria-label={label}
      value={value}
      onChange={(event) => {
        setValue(event.target.value);
      }}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === 'Enter' && value.trim() !== '') onCommit(value.trim());
        if (event.key === 'Escape') onCancel();
      }}
    />
  );
}

/**
 * The 212px navigation rail: saved views, the folder tree and tags — plus
 * their management (create/rename/delete/reorder). The design covers the
 * rail's browse state only; management follows the system language:
 * inline confirmations, red text for destructive actions with real counts,
 * no icons.
 */
export function LibraryRail({
  counts,
  folders,
  tags,
  scope,
  onScopeChange,
  onCreateFolder,
  onRenameFolder,
  onDeleteFolder,
  onReorderFolder,
  subtreeCount,
  onCreateTag,
  onRenameTag,
  onDeleteTag,
}: RailProps) {
  const tr = useTr();
  const [mode, setMode] = useState<RailMode>({ kind: 'idle' });
  const [selectedTagId, setSelectedTagId] = useState<string | null>(null);
  const folderCounts = new Map(counts?.folders.map((f) => [f.folderId, f.count]));
  const idle = () => {
    setMode({ kind: 'idle' });
  };

  return (
    <nav aria-label={tr('Library', '片段库')} className="tv-lib-rail">
      <div className="tv-lib-rail-label">{tr('Views', '视图')}</div>
      {VIEWS.map(({ view, en, zh, countKey }) => (
        <button
          key={view}
          type="button"
          className="tv-lib-rail-item"
          data-active={scope.folderId === null && scope.view === view ? true : undefined}
          onClick={() => {
            onScopeChange({ view, folderId: null });
          }}
        >
          {tr(en, zh)}
          {counts !== null && (
            <span
              className="tv-lib-rail-count"
              data-attention={view === 'unsorted' && counts.unsorted > 0 ? true : undefined}
            >
              {formatCount(counts[countKey])}
            </span>
          )}
        </button>
      ))}

      <div className="tv-lib-rail-label tv-lib-rail-section">
        {tr('Folders', '文件夹')}
        <button
          type="button"
          className="tv-lib-rail-add"
          onClick={() => {
            setMode({ kind: 'new-folder', parentId: null });
          }}
        >
          {tr('New', '新建')}
        </button>
      </div>
      {mode.kind === 'new-folder' && mode.parentId === null && (
        <div className="tv-lib-rail-edit">
          <NameInput
            label={tr('New folder name', '新文件夹名称')}
            onCommit={(name) => {
              onCreateFolder(name, null);
              idle();
            }}
            onCancel={idle}
          />
        </div>
      )}
      {folders.map(({ folder, depth }) => {
        const active = scope.folderId === folder.id;
        return (
          <div key={folder.id}>
            {mode.kind === 'rename-folder' && mode.id === folder.id ? (
              <div className="tv-lib-rail-edit" style={{ paddingLeft: 22 + depth * 12 }}>
                <NameInput
                  label={tr(`Rename ${folder.name}`, `重命名 ${folder.name}`)}
                  defaultValue={folder.name}
                  onCommit={(name) => {
                    onRenameFolder(folder, name);
                    idle();
                  }}
                  onCancel={idle}
                />
              </div>
            ) : (
              <button
                type="button"
                className="tv-lib-rail-item"
                style={{ paddingLeft: 22 + depth * 12 }}
                data-active={active ? true : undefined}
                onClick={() => {
                  onScopeChange({ view: 'folder', folderId: folder.id });
                }}
                onKeyDown={(event) => {
                  if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
                    event.preventDefault();
                    event.stopPropagation();
                    onReorderFolder(folder, event.key === 'ArrowUp' ? -1 : 1);
                  }
                }}
              >
                {folder.name}
                <span className="tv-lib-rail-count">
                  {formatCount(folderCounts.get(folder.id) ?? 0)}
                </span>
              </button>
            )}
            {active && mode.kind === 'idle' && (
              <div className="tv-lib-rail-actions" style={{ paddingLeft: 22 + depth * 12 }}>
                <button
                  type="button"
                  onClick={() => {
                    setMode({ kind: 'rename-folder', id: folder.id });
                  }}
                >
                  {tr('Rename', '重命名')}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setMode({ kind: 'new-folder', parentId: folder.id });
                  }}
                >
                  {tr('New sub', '新建子文件夹')}
                </button>
                <button
                  type="button"
                  className="tv-lib-rail-danger"
                  onClick={() => {
                    setMode({ kind: 'confirm-delete-folder', id: folder.id });
                  }}
                >
                  {tr('Delete', '删除')}
                </button>
              </div>
            )}
            {mode.kind === 'new-folder' && mode.parentId === folder.id && (
              <div className="tv-lib-rail-edit" style={{ paddingLeft: 34 + depth * 12 }}>
                <NameInput
                  label={tr(`New folder inside ${folder.name}`, `在 ${folder.name} 中新建文件夹`)}
                  onCommit={(name) => {
                    onCreateFolder(name, folder.id);
                    idle();
                  }}
                  onCancel={idle}
                />
              </div>
            )}
            {mode.kind === 'confirm-delete-folder' && mode.id === folder.id && (
              <div className="tv-lib-rail-confirm" style={{ paddingLeft: 22 + depth * 12 }}>
                <span>
                  {tr(
                    `Delete “${folder.name}” — ${formatCount(subtreeCount(folder.id))} snippets move out`,
                    `删除「${folder.name}」— ${formatCount(subtreeCount(folder.id))} 个片段将移出`,
                  )}
                </span>
                <span className="tv-lib-rail-confirm-actions">
                  <button
                    type="button"
                    className="tv-lib-rail-danger"
                    onClick={() => {
                      onDeleteFolder(folder);
                      idle();
                    }}
                  >
                    {tr('Delete', '删除')}
                  </button>
                  <button type="button" onClick={idle}>
                    {tr('Cancel', '取消')}
                  </button>
                </span>
              </div>
            )}
          </div>
        );
      })}

      <div className="tv-lib-rail-label tv-lib-rail-section">
        {tr('Tags', '标签')}
        <button
          type="button"
          className="tv-lib-rail-add"
          onClick={() => {
            setMode({ kind: 'new-tag' });
          }}
        >
          {tr('New', '新建')}
        </button>
      </div>
      {mode.kind === 'new-tag' && (
        <div className="tv-lib-rail-edit">
          <NameInput
            label={tr('New tag name', '新标签名称')}
            onCommit={(name) => {
              onCreateTag(name);
              idle();
            }}
            onCancel={idle}
          />
        </div>
      )}
      {mode.kind === 'rename-tag' && (
        <div className="tv-lib-rail-edit">
          <NameInput
            label={tr('Rename tag', '重命名标签')}
            defaultValue={tags.find((tag) => tag.id === mode.id)?.name}
            onCommit={(name) => {
              const tag = tags.find((t) => t.id === mode.id);
              if (tag !== undefined) onRenameTag(tag, name);
              idle();
            }}
            onCancel={idle}
          />
        </div>
      )}
      {tags.length > 0 && (
        <div className="tv-lib-rail-tags">
          {tags.map((tag) => (
            <button
              key={tag.id}
              type="button"
              className="tv-lib-tag"
              data-active={selectedTagId === tag.id ? true : undefined}
              onClick={() => {
                setSelectedTagId(selectedTagId === tag.id ? null : tag.id);
                idle();
              }}
            >
              {tag.name}
            </button>
          ))}
        </div>
      )}
      {selectedTagId !== null &&
        mode.kind !== 'confirm-delete-tag' &&
        mode.kind !== 'rename-tag' && (
          <div className="tv-lib-rail-actions">
            <button
              type="button"
              onClick={() => {
                setMode({ kind: 'rename-tag', id: selectedTagId });
              }}
            >
              {tr('Rename', '重命名')}
            </button>
            <button
              type="button"
              className="tv-lib-rail-danger"
              onClick={() => {
                setMode({ kind: 'confirm-delete-tag', id: selectedTagId });
              }}
            >
              {tr('Delete', '删除')}
            </button>
          </div>
        )}
      {mode.kind === 'confirm-delete-tag' && (
        <div className="tv-lib-rail-confirm">
          <span>
            {tr(
              `Delete tag “${tags.find((tag) => tag.id === mode.id)?.name ?? ''}” — snippets keep their text`,
              `删除标签「${tags.find((tag) => tag.id === mode.id)?.name ?? ''}」— 片段内容保持不变`,
            )}
          </span>
          <span className="tv-lib-rail-confirm-actions">
            <button
              type="button"
              className="tv-lib-rail-danger"
              onClick={() => {
                const tag = tags.find((t) => t.id === mode.id);
                if (tag !== undefined) onDeleteTag(tag);
                setSelectedTagId(null);
                idle();
              }}
            >
              {tr('Delete', '删除')}
            </button>
            <button type="button" onClick={idle}>
              {tr('Cancel', '取消')}
            </button>
          </span>
        </div>
      )}
    </nav>
  );
}
