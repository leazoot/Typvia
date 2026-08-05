import { libraryCounts, listFolderChildren, listTags } from '@typvia/shared';
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
  const [scope, setScope] = useState<LibraryScope>({ view: 'all', folderId: null });
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [batchIds, setBatchIds] = useState<ReadonlySet<string>>(new Set());
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [folders, setFolders] = useState<FolderEntry[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);

  const viewportRef = useRef<HTMLDivElement>(null);
  const { total, rowAt, ensureRange, failed, retry } = useSnippetPages(scope, typeFilter);
  const { first, last, totalHeight } = useVirtualRows({
    count: total,
    rowHeight: ROW_HEIGHT,
    viewportRef,
  });

  useEffect(() => {
    ensureRange(first, last);
  }, [ensureRange, first, last]);

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

  const selectedSnippet = selectedIndex === null ? null : (rowAt(selectedIndex) ?? null);

  const rows = [];
  if (total > 0) {
    for (let index = first; index <= last; index += 1) {
      const snippet = rowAt(index);
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
    <main className="tv-lib">
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
            trailing={total.toLocaleString('en-US')}
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
            ) : total === 0 ? (
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
            Local library · {total.toLocaleString('en-US')} items
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
