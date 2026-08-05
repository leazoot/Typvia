import {
  batchMoveSnippets,
  batchTagSnippets,
  batchTrashSnippets,
  createFolder,
  createTag,
  deleteFolder,
  deleteTag,
  libraryCounts,
  listFolderChildren,
  listTags,
  renameTag,
  searchLibrary,
  updateFolder,
} from '@typvia/shared';
import type { Folder, LibraryCounts, Snippet, Tag } from '@typvia/shared';
import {
  Caret,
  ListTray,
  SearchLine,
  SelectionPlate,
  SnippetRow,
  SnippetRowSkeleton,
  designTokens,
  useVirtualRows,
} from '@typvia/ui';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { markFor, LibraryPreview } from './preview';
import { LibraryRail, type FolderEntry } from './rail';
import { useSnippetPages, type LibraryScope } from './use-snippet-pages';
import './library.css';

const ROW_HEIGHT = designTokens.space.rowHeightDesktop;

/** The rail views repeated as horizontal chips under the 1160 breakpoint. */
const BASE_SCOPE_CHIPS: Array<{ label: string; scope: LibraryScope }> = [
  { label: 'All', scope: { view: 'all', folderId: null } },
  { label: 'Recent', scope: { view: 'recent', folderId: null } },
  { label: 'Starred', scope: { view: 'starred', folderId: null } },
];

/** The type-filter chips (design 1b). `null` = all types. */
const TYPE_CHIPS: Array<{ label: string; value: string | null }> = [
  { label: 'All types', value: null },
  { label: 'Command', value: 'command' },
  { label: 'Template', value: 'template' },
  { label: 'Secret', value: 'sensitive' },
  { label: 'AI action', value: 'ai_action' },
];

/** Query handed over from the Home search line via router state. */
function initialQueryFrom(state: unknown): string {
  if (
    typeof state === 'object' &&
    state !== null &&
    'query' in state &&
    typeof state.query === 'string'
  ) {
    return state.query;
  }
  return '';
}

async function fetchFolderTree(): Promise<FolderEntry[]> {
  const entries: FolderEntry[] = [];
  async function walk(parentId: string | null, depth: number): Promise<void> {
    const children = await listFolderChildren(parentId);
    for (const folder of children) {
      entries.push({ folder, depth });
      await walk(folder.id, depth + 1);
    }
  }
  await walk(null, 0);
  return entries;
}

/**
 * Library (design Phase 2 · 1b): rail → search line → filter chips → 52px
 * virtualised rows on the sunken tray → read-only preview. Live search
 * filtering, keyboard selection and batch mutations arrive with
 * TASK-031/032; this screen owns layout, scoping, virtual scrolling,
 * selection and batch-selection state.
 */
export function LibraryPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const [scope, setScope] = useState<LibraryScope>({ view: 'all', folderId: null });
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [query, setQuery] = useState(() => initialQueryFrom(location.state));
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [batchIds, setBatchIds] = useState<ReadonlySet<string>>(new Set());
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [folders, setFolders] = useState<FolderEntry[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  // null = browsing the scope; an array = live search results for `query`.
  const [results, setResults] = useState<Snippet[] | null>(null);
  const [searchMs, setSearchMs] = useState<number | null>(null);
  const searchGeneration = useRef(0);

  const viewportRef = useRef<HTMLDivElement>(null);
  const { total, rowAt, ensureRange, failed, retry } = useSnippetPages(scope, typeFilter);

  // Every keystroke replaces the results instantly — no debounce, no
  // transition (design 1b). A generation stamp drops stale responses.
  useEffect(() => {
    searchGeneration.current += 1;
    const generation = searchGeneration.current;
    if (query.trim() === '') {
      setResults(null);
      setSearchMs(null);
      return;
    }
    const started = performance.now();
    searchLibrary(query, 500)
      .then((rows) => {
        if (searchGeneration.current !== generation) return;
        setResults(rows);
        setSearchMs(performance.now() - started);
        setSelectedIndex(rows.length > 0 ? 0 : null);
      })
      .catch(() => {
        if (searchGeneration.current === generation) setResults([]);
      });
  }, [query]);

  const searchRows = useMemo(() => {
    if (results === null) return null;
    return typeFilter === null
      ? results
      : results.filter((snippet) => snippet.snippetType === typeFilter);
  }, [results, typeFilter]);

  const rowCount = searchRows === null ? total : searchRows.length;
  const rowSource = (index: number): Snippet | undefined =>
    searchRows === null ? rowAt(index) : searchRows[index];

  const { first, last, totalHeight } = useVirtualRows({
    count: rowCount,
    rowHeight: ROW_HEIGHT,
    viewportRef,
  });

  useEffect(() => {
    if (searchRows === null) ensureRange(first, last);
  }, [ensureRange, first, last, searchRows]);

  const refreshMeta = () => {
    libraryCounts()
      .then(setCounts)
      .catch(() => {
        // The list area reports load failures; the rail just stays uncounted.
      });
    fetchFolderTree()
      .then(setFolders)
      .catch(() => {
        setFolders([]);
      });
    listTags()
      .then(setTags)
      .catch(() => {
        setTags([]);
      });
  };

  useEffect(refreshMeta, []);

  /** Re-reads rows and rail numbers after any mutation. */
  const refreshAll = () => {
    retry();
    refreshMeta();
  };

  const folderNames = useMemo(
    () => new Map(folders.map(({ folder }) => [folder.id, folder.name])),
    [folders],
  );

  const selectScope = (next: LibraryScope) => {
    setScope(next);
    setSelectedIndex(null);
    setBatchIds(new Set());
    if (viewportRef.current !== null) viewportRef.current.scrollTop = 0;
  };

  const toggleBatch = (snippet: Snippet, included: boolean) => {
    setBatchIds((previous) => {
      const next = new Set(previous);
      if (included) next.add(snippet.id);
      else next.delete(snippet.id);
      return next;
    });
  };

  const selectedSnippet = selectedIndex === null ? null : (rowSource(selectedIndex) ?? null);

  /** ↑↓ move the selection (viewport follows); ↵ opens the selected row. */
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp' && event.key !== 'Enter') return;
    if (event.key === 'Enter') {
      if (selectedSnippet !== null) {
        event.preventDefault();
        void navigate(`/editor/${selectedSnippet.id}`);
      }
      return;
    }
    if (rowCount === 0) return;
    event.preventDefault();
    const next =
      event.key === 'ArrowDown'
        ? Math.min((selectedIndex ?? -1) + 1, rowCount - 1)
        : Math.max((selectedIndex ?? 1) - 1, 0);
    setSelectedIndex(next);
    const viewport = viewportRef.current;
    if (viewport !== null) {
      const rowTop = next * ROW_HEIGHT;
      if (rowTop < viewport.scrollTop) viewport.scrollTop = rowTop;
      else if (rowTop + ROW_HEIGHT > viewport.scrollTop + viewport.clientHeight)
        viewport.scrollTop = rowTop + ROW_HEIGHT - viewport.clientHeight;
    }
  };

  const clearFilters = () => {
    setQuery('');
    setTypeFilter(null);
    setSelectedIndex(null);
  };

  // ---- folder / tag management (rail callbacks) -------------------------

  const childrenOf = useMemo(() => {
    const map = new Map<string | null, FolderEntry[]>();
    for (const entry of folders) {
      const key = entry.folder.parentId;
      map.set(key, [...(map.get(key) ?? []), entry]);
    }
    return map;
  }, [folders]);

  /** Live snippet count of a folder plus all of its descendants. */
  const subtreeCount = (folderId: string): number => {
    const direct = new Map(counts?.folders.map((f) => [f.folderId, f.count]));
    let sum = 0;
    const queue = [folderId];
    while (queue.length > 0) {
      const current = queue.pop();
      if (current === undefined) break;
      sum += direct.get(current) ?? 0;
      for (const child of childrenOf.get(current) ?? []) queue.push(child.folder.id);
    }
    return sum;
  };

  const run = (operation: Promise<unknown>) => {
    operation.then(refreshAll).catch(refreshAll);
  };

  const handleReorderFolder = (folder: Folder, direction: -1 | 1) => {
    const siblings = (childrenOf.get(folder.parentId) ?? []).map((entry) => entry.folder);
    const index = siblings.findIndex((sibling) => sibling.id === folder.id);
    const swapWith = index + direction;
    if (index < 0 || swapWith < 0 || swapWith >= siblings.length) return;
    const reordered = [...siblings];
    const moved = reordered[index];
    const other = reordered[swapWith];
    if (moved === undefined || other === undefined) return;
    reordered[index] = other;
    reordered[swapWith] = moved;
    // Persist a clean sequential order so future swaps stay stable.
    run(
      Promise.all(
        reordered.map((sibling, position) =>
          sibling.sortOrder === position
            ? Promise.resolve()
            : updateFolder({
                id: sibling.id,
                name: sibling.name,
                parentId: sibling.parentId,
                sortOrder: position,
              }),
        ),
      ),
    );
  };

  // ---- batch bar --------------------------------------------------------

  const [batchMenu, setBatchMenu] = useState<'none' | 'move' | 'tag' | 'confirm-delete'>('none');
  const batchDone = () => {
    setBatchIds(new Set());
    setBatchMenu('none');
    setSelectedIndex(null);
    refreshAll();
  };
  const runBatch = (operation: Promise<void>) => {
    operation.then(batchDone).catch(batchDone);
  };

  const rows = [];
  if (rowCount > 0) {
    for (let index = first; index <= last; index += 1) {
      const snippet = rowSource(index);
      rows.push(
        <div key={index} className="tv-lib-row-slot" style={{ top: index * ROW_HEIGHT }}>
          {snippet === undefined ? (
            <SnippetRowSkeleton />
          ) : (
            <SnippetRow
              mark={markFor(snippet.snippetType)}
              title={snippet.title}
              cn={snippet.description ?? ''}
              preview={snippet.body ?? ''}
              trigger={snippet.trigger ?? ''}
              folder={folderNames.get(snippet.folderId ?? '') ?? ''}
              selected={selectedIndex === index}
              onClick={() => {
                setSelectedIndex(index);
              }}
              checked={batchIds.has(snippet.id)}
              onCheckedChange={(checked) => {
                toggleBatch(snippet, checked);
              }}
            />
          )}
        </div>,
      );
    }
  }

  return (
    // Keyboard list navigation works from anywhere on the page, including
    // while typing in the search line (design: type to filter; ↑↓ ↵).
    <main className="tv-lib" onKeyDown={handleKeyDown}>
      <LibraryRail
        counts={counts}
        folders={folders}
        tags={tags}
        scope={scope}
        onScopeChange={selectScope}
        onCreateFolder={(name, parentId) => {
          run(createFolder({ name, parentId, sortOrder: folders.length }));
        }}
        onRenameFolder={(folder, name) => {
          run(
            updateFolder({
              id: folder.id,
              name,
              parentId: folder.parentId,
              sortOrder: folder.sortOrder,
            }),
          );
        }}
        onDeleteFolder={(folder) => {
          if (scope.folderId === folder.id) setScope({ view: 'all', folderId: null });
          run(deleteFolder(folder.id));
        }}
        onReorderFolder={handleReorderFolder}
        subtreeCount={subtreeCount}
        onCreateTag={(name) => {
          run(createTag(name));
        }}
        onRenameTag={(tag, name) => {
          run(renameTag(tag.id, name));
        }}
        onDeleteTag={(tag) => {
          run(deleteTag(tag.id));
        }}
      />
      <section className="tv-lib-main">
        <div className="tv-lib-head">
          <SearchLine
            scale="library"
            value={query}
            onChange={setQuery}
            placeholder="Search titles, content, triggers, tags…"
            label="Search snippets"
            trailing={
              <span aria-live="polite" role="status">
                {searchRows === null
                  ? total.toLocaleString('en-US')
                  : `${String(searchRows.length)} results`}
              </span>
            }
          />
          <div className="tv-lib-scopes">
            {[
              ...BASE_SCOPE_CHIPS,
              ...folders
                .filter(({ depth }) => depth === 0)
                .map(({ folder }): { label: string; scope: LibraryScope } => ({
                  label: folder.name,
                  scope: { view: 'folder', folderId: folder.id },
                })),
            ].map(({ label, scope: chipScope }) => (
              <button
                key={label}
                type="button"
                className="tv-lib-chip"
                data-active={
                  scope.view === chipScope.view && scope.folderId === chipScope.folderId
                    ? true
                    : undefined
                }
                onClick={() => {
                  selectScope(chipScope);
                }}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="tv-lib-chips">
            {TYPE_CHIPS.map((chip) => (
              <button
                key={chip.label}
                type="button"
                className="tv-lib-chip"
                data-active={typeFilter === chip.value ? true : undefined}
                onClick={() => {
                  setTypeFilter(chip.value);
                  setSelectedIndex(null);
                  if (viewportRef.current !== null) viewportRef.current.scrollTop = 0;
                }}
              >
                {chip.label}
              </button>
            ))}
            <span className="tv-lib-chip-divider" />
            <span className="tv-lib-sort">Sort: last updated</span>
          </div>
        </div>
        <ListTray className="tv-lib-list">
          <div ref={viewportRef} className="tv-lib-viewport" data-testid="library-viewport">
            {failed ? (
              <div className="tv-lib-state">
                <div className="tv-lib-state-title">Your snippets are safe on this device.</div>
                <div className="tv-lib-state-text">This view failed to load just now.</div>
                <button type="button" className="tv-lib-state-action" onClick={retry}>
                  Retry
                </button>
              </div>
            ) : searchRows !== null && rowCount === 0 ? (
              <div className="tv-lib-state tv-lib-state-center">
                <div className="tv-lib-state-figure">
                  <span className="tv-lib-state-bar" />
                  <Caret height={17} />
                </div>
                <div className="tv-lib-state-title">No snippet matches that</div>
                <div className="tv-lib-state-text">
                  Save what you typed as a new snippet, or clear the filters.
                </div>
                <div className="tv-lib-state-actions">
                  <button
                    type="button"
                    className="tv-lib-state-primary"
                    onClick={() => void navigate('/editor', { state: { draftTitle: query } })}
                  >
                    Save as snippet
                  </button>
                  <button type="button" className="tv-lib-state-action" onClick={clearFilters}>
                    Clear filters
                  </button>
                </div>
              </div>
            ) : rowCount === 0 ? (
              <div className="tv-lib-state">
                <div className="tv-lib-state-figure">
                  <span className="tv-lib-state-slot" />
                  <Caret height={16} />
                </div>
                <div className="tv-lib-state-title">
                  {scope.folderId !== null
                    ? `${folderNames.get(scope.folderId) ?? 'This folder'} is empty`
                    : 'Nothing here yet'}
                </div>
                <div className="tv-lib-state-text">
                  {scope.folderId !== null
                    ? 'Move snippets here from any list.'
                    : 'Snippets you save will appear here as rows.'}
                </div>
              </div>
            ) : (
              <div className="tv-lib-spacer" style={{ height: totalHeight }}>
                <SelectionPlate index={selectedIndex} />
                {rows}
              </div>
            )}
          </div>
        </ListTray>
        <footer className="tv-lib-foot">
          {batchIds.size > 0 &&
            (batchMenu === 'confirm-delete' ? (
              <>
                <span className="tv-lib-foot-selected">
                  Delete {batchIds.size} snippet{batchIds.size === 1 ? '' : 's'}?
                </span>
                <button
                  type="button"
                  className="tv-lib-foot-action tv-lib-foot-danger"
                  onClick={() => {
                    runBatch(batchTrashSnippets([...batchIds]));
                  }}
                >
                  Delete {batchIds.size} snippet{batchIds.size === 1 ? '' : 's'}
                </button>
                <button
                  type="button"
                  className="tv-lib-foot-action"
                  onClick={() => {
                    setBatchMenu('none');
                  }}
                >
                  Cancel
                </button>
              </>
            ) : (
              <>
                <span className="tv-lib-foot-selected">{batchIds.size} selected</span>
                <span className="tv-lib-foot-divider" />
                <span className="tv-lib-foot-menu-anchor">
                  <button
                    type="button"
                    className="tv-lib-foot-action"
                    onClick={() => {
                      setBatchMenu(batchMenu === 'move' ? 'none' : 'move');
                    }}
                  >
                    Move to folder
                  </button>
                  {batchMenu === 'move' && (
                    <div className="tv-lib-popover" role="listbox" aria-label="Move to folder">
                      <button
                        type="button"
                        className="tv-lib-popover-item"
                        onClick={() => {
                          runBatch(batchMoveSnippets([...batchIds], null));
                        }}
                      >
                        Unsorted
                      </button>
                      {folders.map(({ folder, depth }) => (
                        <button
                          key={folder.id}
                          type="button"
                          className="tv-lib-popover-item"
                          style={{ paddingLeft: 12 + depth * 12 }}
                          onClick={() => {
                            runBatch(batchMoveSnippets([...batchIds], folder.id));
                          }}
                        >
                          {folder.name}
                        </button>
                      ))}
                    </div>
                  )}
                </span>
                <span className="tv-lib-foot-menu-anchor">
                  <button
                    type="button"
                    className="tv-lib-foot-action"
                    onClick={() => {
                      setBatchMenu(batchMenu === 'tag' ? 'none' : 'tag');
                    }}
                  >
                    Add tag
                  </button>
                  {batchMenu === 'tag' && (
                    <div className="tv-lib-popover" role="listbox" aria-label="Add tag">
                      {tags.length === 0 ? (
                        <span className="tv-lib-popover-empty">
                          No tags yet — create one in the rail.
                        </span>
                      ) : (
                        tags.map((tag) => (
                          <button
                            key={tag.id}
                            type="button"
                            className="tv-lib-popover-item"
                            onClick={() => {
                              runBatch(batchTagSnippets([...batchIds], tag.id));
                            }}
                          >
                            {tag.name}
                          </button>
                        ))
                      )}
                    </div>
                  )}
                </span>
                <button
                  type="button"
                  className="tv-lib-foot-action tv-lib-foot-danger"
                  onClick={() => {
                    setBatchMenu('confirm-delete');
                  }}
                >
                  Delete
                </button>
              </>
            ))}
          <span className="tv-lib-foot-count">
            {searchRows === null || searchMs === null
              ? `Local library · ${total.toLocaleString('en-US')} items`
              : `Local search across ${total.toLocaleString('en-US')} items · ${searchMs.toFixed(1)} ms`}
          </span>
        </footer>
      </section>
      <LibraryPreview
        snippet={selectedSnippet}
        folderName={
          selectedSnippet === null
            ? null
            : (folderNames.get(selectedSnippet.folderId ?? '') ?? null)
        }
        onClose={() => {
          setSelectedIndex(null);
        }}
        onCopied={refreshAll}
      />
    </main>
  );
}
