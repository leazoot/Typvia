import type { Folder, LibraryCounts, Tag } from '@typvia/shared';
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

interface RailProps {
  counts: LibraryCounts | null;
  folders: FolderEntry[];
  tags: Tag[];
  scope: LibraryScope;
  onScopeChange: (scope: LibraryScope) => void;
}

function formatCount(count: number): string {
  return count.toLocaleString('en-US');
}

/**
 * The 212px navigation rail: saved views, the folder tree and tags.
 * Navigation only — content lives in the list (design 1b).
 */
export function LibraryRail({ counts, folders, tags, scope, onScopeChange }: RailProps) {
  const folderCounts = new Map(counts?.folders.map((f) => [f.folderId, f.count]));
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
      {folders.length > 0 && <div className="tv-lib-rail-label tv-lib-rail-section">Folders</div>}
      {folders.map(({ folder, depth }) => (
        <button
          key={folder.id}
          type="button"
          className="tv-lib-rail-item"
          style={{ paddingLeft: 22 + depth * 12 }}
          data-active={scope.folderId === folder.id ? true : undefined}
          onClick={() => {
            onScopeChange({ view: 'folder', folderId: folder.id });
          }}
        >
          {folder.name}
          <span className="tv-lib-rail-count">{formatCount(folderCounts.get(folder.id) ?? 0)}</span>
        </button>
      ))}
      {tags.length > 0 && (
        <>
          <div className="tv-lib-rail-label tv-lib-rail-section">Tags</div>
          <div className="tv-lib-rail-tags">
            {tags.map((tag) => (
              <span key={tag.id} className="tv-lib-tag">
                {tag.name}
              </span>
            ))}
          </div>
        </>
      )}
    </nav>
  );
}
