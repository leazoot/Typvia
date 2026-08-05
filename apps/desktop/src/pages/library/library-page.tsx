import { libraryCounts, listFolderChildren, listTags, searchLibrary } from '@typvia/shared';
import type { LibraryCounts, Snippet, Tag } from '@typvia/shared';
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
import { useNavigate } from 'react-router';
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
  const [scope, setScope] = useState<LibraryScope>({ view: 'all', folderId: null });
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [query, setQuery] = useState('');
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

  useEffect(() => {
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
  }, []);

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
          {batchIds.size > 0 && (
            <>
              <span className="tv-lib-foot-selected">{batchIds.size} selected</span>
              <span className="tv-lib-foot-divider" />
              <button type="button" className="tv-lib-foot-action" disabled>
                Move to folder
              </button>
              <button type="button" className="tv-lib-foot-action" disabled>
                Add tag
              </button>
              <button type="button" className="tv-lib-foot-action tv-lib-foot-danger" disabled>
                Delete
              </button>
            </>
          )}
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
      />
    </main>
  );
}
