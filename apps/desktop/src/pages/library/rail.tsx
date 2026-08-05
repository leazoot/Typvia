import type { Folder, LibraryCounts, Tag } from '@typvia/shared';
import { useState } from 'react';
import type { LibraryScope } from './use-snippet-pages';

/** A folder flattened out of the tree with its indent depth. */
export interface FolderEntry {
  folder: Folder;
  depth: number;
}

type CountKey = 'total' | 'recent' | 'starred' | 'unsorted';

const VIEWS: Array<{ view: LibraryScope['view']; label: string; countKey: CountKey }> = [
  { view: 'all', label: 'All snippets', countKey: 'total' },
  { view: 'recent', label: 'Recent', countKey: 'recent' },
  { view: 'starred', label: 'Starred', countKey: 'starred' },
  { view: 'unsorted', label: 'Unsorted', countKey: 'unsorted' },
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
  const [mode, setMode] = useState<RailMode>({ kind: 'idle' });
  const [selectedTagId, setSelectedTagId] = useState<string | null>(null);
  const folderCounts = new Map(counts?.folders.map((f) => [f.folderId, f.count]));
  const idle = () => {
    setMode({ kind: 'idle' });
  };

  return (
    <nav aria-label="Library" className="tv-lib-rail">
      <div className="tv-lib-rail-label">Views</div>
      {VIEWS.map(({ view, label, countKey }) => (
        <button
          key={view}
          type="button"
          className="tv-lib-rail-item"
          data-active={scope.folderId === null && scope.view === view ? true : undefined}
          onClick={() => {
            onScopeChange({ view, folderId: null });
          }}
        >
          {label}
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
        Folders
        <button
          type="button"
          className="tv-lib-rail-add"
          onClick={() => {
            setMode({ kind: 'new-folder', parentId: null });
          }}
        >
          New
        </button>
      </div>
      {mode.kind === 'new-folder' && mode.parentId === null && (
        <div className="tv-lib-rail-edit">
          <NameInput
            label="New folder name"
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
                  label={`Rename ${folder.name}`}
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
                  Rename
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setMode({ kind: 'new-folder', parentId: folder.id });
                  }}
                >
                  New sub
                </button>
                <button
                  type="button"
                  className="tv-lib-rail-danger"
                  onClick={() => {
                    setMode({ kind: 'confirm-delete-folder', id: folder.id });
                  }}
                >
                  Delete
                </button>
              </div>
            )}
            {mode.kind === 'new-folder' && mode.parentId === folder.id && (
              <div className="tv-lib-rail-edit" style={{ paddingLeft: 34 + depth * 12 }}>
                <NameInput
                  label={`New folder inside ${folder.name}`}
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
                  {`Delete “${folder.name}” — ${formatCount(subtreeCount(folder.id))} snippets move out`}
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
                    Delete
                  </button>
                  <button type="button" onClick={idle}>
                    Cancel
                  </button>
                </span>
              </div>
            )}
          </div>
        );
      })}

      <div className="tv-lib-rail-label tv-lib-rail-section">
        Tags
        <button
          type="button"
          className="tv-lib-rail-add"
          onClick={() => {
            setMode({ kind: 'new-tag' });
          }}
        >
          New
        </button>
      </div>
      {mode.kind === 'new-tag' && (
        <div className="tv-lib-rail-edit">
          <NameInput
            label="New tag name"
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
            label="Rename tag"
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
              Rename
            </button>
            <button
              type="button"
              className="tv-lib-rail-danger"
              onClick={() => {
                setMode({ kind: 'confirm-delete-tag', id: selectedTagId });
              }}
            >
              Delete
            </button>
          </div>
        )}
      {mode.kind === 'confirm-delete-tag' && (
        <div className="tv-lib-rail-confirm">
          <span>{`Delete tag “${tags.find((tag) => tag.id === mode.id)?.name ?? ''}” — snippets keep their text`}</span>
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
              Delete
            </button>
            <button type="button" onClick={idle}>
              Cancel
            </button>
          </span>
        </div>
      )}
    </nav>
  );
}
