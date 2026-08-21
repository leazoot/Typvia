// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import {
  batchMoveSnippets,
  batchTagSnippets,
  batchTrashSnippets,
  copySnippet,
  createFolder,
  createSnippet,
  createTag,
  deleteFolder,
  deleteTag,
  libraryCounts,
  listFolderChildren,
  listTags,
  renameTag,
  searchLibraryDeep,
  snippetConvertToSensitive,
  trashSnippet,
  updateFolder,
  updateSnippet,
} from '@typvia/shared';
import type { Folder, LibraryCounts, Snippet, Tag } from '@typvia/shared';
import { TypeMark, markForType, useTr, useVirtualRows } from '@typvia/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { useEspanso } from '../../espanso/espanso-context';
import {
  Action,
  CopyAction,
  Glyph,
  OverflowMenu,
  Row,
  TriggerToken,
  useContextMenu,
  type MenuEntry,
} from '../../workspace/kit';
import { LibraryRail, type FolderEntry } from './rail';
import { useSnippetPages, type LibraryScope } from './use-snippet-pages';
import './library.css';

const ROW_HEIGHT = 52;
/** Height the peeked row borrows from the list while it is open. */
const PEEK_HEIGHT = 176;
const SEARCH_LIMIT = 500;

/** The three views that stay visible; the rest live in the filter popover. */
const TABS: Array<{ en: string; zh: string; view: LibraryScope['view'] }> = [
  { en: 'All', zh: '全部', view: 'all' },
  { en: 'Recent', zh: '最近', view: 'recent' },
  { en: 'Starred', zh: '星标', view: 'starred' },
];

const TYPES: Array<{ en: string; zh: string; value: string | null }> = [
  { en: 'All types', zh: '全部类型', value: null },
  { en: 'Command', zh: '命令', value: 'command' },
  { en: 'Template', zh: '模板', value: 'template' },
  { en: 'AI action', zh: 'AI 动作', value: 'ai_action' },
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
 * Library — browsing one's own text, not administering
 * records. The header carries a title, a search line and two folded tools;
 * a row opens in place; selection, deletion and organisation only appear
 * once the user reaches for them.
 */
export function LibraryPage() {
  const tr = useTr();
  const navigate = useNavigate();
  const location = useLocation();
  // Trashing removes triggers from the espanso config: regenerate.
  const { notifyMutation } = useEspanso();

  const [scope, setScope] = useState<LibraryScope>({ view: 'all', folderId: null });
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [query, setQuery] = useState(() => initialQueryFrom(location.state));
  const [results, setResults] = useState<Snippet[] | null>(null);
  const [peekIndex, setPeekIndex] = useState<number | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [filterOpen, setFilterOpen] = useState(false);
  const [organiseOpen, setOrganiseOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [batchMenu, setBatchMenu] = useState<'none' | 'move' | 'tag'>('none');
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [folders, setFolders] = useState<FolderEntry[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);

  const viewportRef = useRef<HTMLDivElement>(null);
  const filterWrap = useRef<HTMLSpanElement>(null);
  const searchGeneration = useRef(0);
  const context = useContextMenu(tr('Snippet actions', '片段操作'));
  const { total, rowAt, ensureRange, failed, retry } = useSnippetPages(scope, typeFilter);

  // Every keystroke replaces the results instantly — no debounce.
  useEffect(() => {
    searchGeneration.current += 1;
    const generation = searchGeneration.current;
    setPeekIndex(null);
    if (query.trim() === '') {
      setResults(null);
      return;
    }
    searchLibraryDeep(query, SEARCH_LIMIT)
      .then((rows) => {
        if (searchGeneration.current === generation) setResults(rows);
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
  const rowSource = useCallback(
    (index: number): Snippet | undefined =>
      searchRows === null ? rowAt(index) : searchRows[index],
    [searchRows, rowAt],
  );

  const { first, last, totalHeight } = useVirtualRows({
    count: rowCount,
    rowHeight: ROW_HEIGHT,
    viewportRef,
  });
  // The open peek pushes the rows below it down, so a few extra rows are
  // rendered past the measured window.
  const lastRendered = Math.min(rowCount - 1, last + 3);

  useEffect(() => {
    if (searchRows === null) ensureRange(first, lastRendered);
  }, [ensureRange, first, lastRendered, searchRows]);

  const refreshMeta = useCallback(() => {
    libraryCounts()
      .then(setCounts)
      .catch(() => {
        // The list area reports load failures; the header just stays uncounted.
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
  useEffect(refreshMeta, [refreshMeta]);

  const refreshAll = useCallback(() => {
    retry();
    refreshMeta();
  }, [retry, refreshMeta]);

  const run = (operation: Promise<unknown>) => {
    operation.then(refreshAll).catch(refreshAll);
  };

  useEffect(() => {
    if (!filterOpen) return;
    const away = (event: MouseEvent) => {
      if (filterWrap.current?.contains(event.target as Node) !== true) setFilterOpen(false);
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [filterOpen]);

  const folderNames = useMemo(
    () => new Map(folders.map(({ folder }) => [folder.id, folder.name])),
    [folders],
  );

  const selectScope = (next: LibraryScope) => {
    setScope(next);
    setPeekIndex(null);
    setSelected(new Set());
    if (viewportRef.current !== null) viewportRef.current.scrollTop = 0;
  };

  const clearFilters = () => {
    setQuery('');
    setTypeFilter(null);
    selectScope({ view: 'all', folderId: null });
  };

  const activeFilters =
    (typeFilter === null ? 0 : 1) + (scope.view === 'all' || scope.view === 'folder' ? 0 : 1);

  // ---- selection --------------------------------------------------------

  const toggleSelected = (id: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectRange = (from: number, to: number) => {
    const [start, end] = from <= to ? [from, to] : [to, from];
    setSelected((current) => {
      const next = new Set(current);
      for (let index = start; index <= end; index += 1) {
        const snippet = rowSource(index);
        if (snippet !== undefined) next.add(snippet.id);
      }
      return next;
    });
  };

  const clearSelection = () => {
    setSelected(new Set());
    setBatchMenu('none');
    setConfirmDelete(false);
  };

  const batchDone = () => {
    clearSelection();
    setPeekIndex(null);
    refreshAll();
  };
  const runBatch = (operation: Promise<void>) => {
    operation.then(batchDone).catch(batchDone);
  };

  // ---- single-row actions ----------------------------------------------

  const duplicate = (snippet: Snippet) => {
    run(
      createSnippet({
        title: tr(`${snippet.title} copy`, `${snippet.title} 副本`),
        body: snippet.body ?? '',
        snippetType: snippet.snippetType,
        description: snippet.description,
        folderId: snippet.folderId,
        // A trigger is unique, so the copy starts without one.
        trigger: null,
        triggerMode: snippet.triggerMode,
        language: snippet.language,
      }),
    );
  };

  const toggleFavorite = (snippet: Snippet) => {
    run(
      updateSnippet({
        id: snippet.id,
        title: snippet.title,
        body: snippet.body ?? '',
        snippetType: snippet.snippetType,
        description: snippet.description,
        folderId: snippet.folderId,
        trigger: snippet.trigger,
        triggerMode: snippet.triggerMode,
        language: snippet.language,
        isFavorite: !snippet.isFavorite,
        isPinned: snippet.isPinned,
        isEnabled: snippet.isEnabled,
      }),
    );
  };

  const menuFor = (snippet: Snippet): MenuEntry[] => {
    const items: MenuEntry[] = [
      { label: tr('Duplicate', '创建副本'), onSelect: () => duplicate(snippet) },
      {
        label: snippet.isFavorite ? tr('Remove star', '取消星标') : tr('Star', '加星标'),
        onSelect: () => toggleFavorite(snippet),
      },
    ];
    if (snippet.securityLevel !== 'sensitive') {
      items.push('divider', {
        label: tr('Move to Vault', '移入保险库'),
        onSelect: () => run(snippetConvertToSensitive(snippet.id).then(notifyMutation)),
      });
    }
    items.push('divider', {
      label: tr('Move to Trash', '移到回收站'),
      danger: true,
      onSelect: () => run(trashSnippet(snippet.id).then(notifyMutation)),
    });
    return items;
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

  const subtreeCount = (folderId: string): number => {
    const direct = new Map(counts?.folders.map((folder) => [folder.folderId, folder.count]));
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

  // ---- rows -------------------------------------------------------------

  const offsetFor = (index: number) =>
    index * ROW_HEIGHT + (peekIndex !== null && index > peekIndex ? PEEK_HEIGHT : 0);

  const peeked = peekIndex === null ? undefined : rowSource(peekIndex);

  const slots = [];
  if (rowCount > 0 && !failed) {
    for (let index = first; index <= lastRendered; index += 1) {
      const snippet = rowSource(index);
      const rowIndex = index;
      slots.push(
        <div
          key={index}
          className="tvl-slot"
          style={{ transform: `translateY(${offsetFor(index)}px)` }}
        >
          {snippet === undefined ? (
            <div className="tvw-row tvl-row" aria-hidden="true">
              <span className="tvw-grow" />
            </div>
          ) : (
            <Row
              className={`tvl-row ${selected.has(snippet.id) ? 'is-selected' : ''} ${
                peekIndex === rowIndex ? 'is-open' : ''
              }`}
              label={snippet.title}
              onOpen={(modifiers) => {
                // ⌘-click selects, ⇧-click extends; a plain click peeks.
                if (modifiers.meta || (modifiers.shift && anchor !== null)) {
                  setPeekIndex(null);
                  if (modifiers.shift && anchor !== null) selectRange(anchor, rowIndex);
                  else toggleSelected(snippet.id);
                  setAnchor(rowIndex);
                  return;
                }
                setAnchor(rowIndex);
                setPeekIndex((current) => (current === rowIndex ? null : rowIndex));
              }}
              onContextMenu={(event) => context.open(event, menuFor(snippet))}
            >
              <TypeMark code={markForType(snippet.snippetType)} />
              <span className="tvw-grow">
                <span className="tvw-row-title">{snippet.title}</span>
                <span aria-hidden="true" className="tvw-row-preview">
                  {snippet.body ?? tr('Secret', '密文')}
                </span>
              </span>
              <span className="tvl-tail">
                <span className="tvw-row-rest">
                  {snippet.isFavorite && (
                    <span className="tvl-star" role="img" aria-label={tr('Starred', '已加星标')}>
                      ★
                    </span>
                  )}
                  {snippet.trigger !== null && <TriggerToken trigger={snippet.trigger} />}
                  <span className="tvl-uses">{`${String(snippet.usageCount)}×`}</span>
                </span>
                <span className="tvw-hover-actions tvl-actions">
                  {snippet.body !== null && (
                    <CopyAction
                      label={tr('Copy', '复制')}
                      doneLabel={tr('Copied', '已复制')}
                      onCopy={() => copySnippet(snippet.id).then(refreshAll)}
                    />
                  )}
                  <Action
                    label={tr('Edit', '编辑')}
                    onRun={() =>
                      void navigate(
                        snippet.securityLevel === 'sensitive' ? '/vault' : `/editor/${snippet.id}`,
                      )
                    }
                  />
                  <Action
                    label={snippet.isFavorite ? '★' : '☆'}
                    onRun={() => toggleFavorite(snippet)}
                  />
                  <OverflowMenu label={tr('More actions', '更多操作')} items={menuFor(snippet)} />
                </span>
              </span>
            </Row>
          )}
        </div>,
      );
    }
  }

  return (
    <div
      className="tvl"
      onKeyDown={(event) => {
        // ↑↓ walk the rendered rows from anywhere on the page, including the
        // search line; ↵ on a row peeks it (hover is never the only way).
        if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
        const rows = [...(viewportRef.current?.querySelectorAll<HTMLElement>('.tvl-row') ?? [])];
        if (rows.length === 0) return;
        event.preventDefault();
        const current = rows.indexOf(document.activeElement as HTMLElement);
        const next =
          event.key === 'ArrowDown'
            ? Math.min(current + 1, rows.length - 1)
            : Math.max(current - 1, 0);
        rows[current === -1 ? 0 : next]?.focus();
      }}
    >
      <div className="tvl-head">
        <h1>{tr('Library', '片段库')}</h1>
        <span className="tvl-count">
          {searchRows === null
            ? total.toLocaleString('en-US')
            : tr(`${String(rowCount)} results`, `${String(rowCount)} 条结果`)}
        </span>
      </div>

      <div className="tvl-search">
        <Glyph name="search" />
        <input
          type="text"
          value={query}
          aria-label={tr('Search snippets', '搜索片段')}
          placeholder={tr(
            'Search titles, content, triggers, tags…',
            '搜索标题、内容、触发词、标签…',
          )}
          onChange={(event) => setQuery(event.target.value)}
        />
        <span ref={filterWrap} className="tvw-menu-wrap tvl-tools">
          <button
            type="button"
            className="tvw-chip"
            aria-expanded={filterOpen}
            onClick={() => setFilterOpen((open) => !open)}
          >
            {tr('Filter', '筛选')}
            {activeFilters > 0 && <span className="tvl-filter-count">{activeFilters}</span>}
          </button>
          <OverflowMenu
            label={tr('View options', '视图选项')}
            items={[
              {
                label: organiseOpen
                  ? tr('Hide folders & tags', '收起文件夹与标签')
                  : tr('Folders & tags', '文件夹与标签'),
                onSelect: () => setOrganiseOpen((open) => !open),
              },
              { label: tr('New snippet', '新建片段'), onSelect: () => void navigate('/editor') },
            ]}
          />
          {filterOpen && (
            <span className="tvw-pop tvl-pop" aria-label={tr('Filter', '筛选')}>
              <span className="tvw-pop-label">{tr('Type', '类型')}</span>
              <span className="tvl-pop-group">
                {TYPES.map((type) => (
                  <button
                    key={type.en}
                    type="button"
                    className="tvl-choice"
                    aria-pressed={typeFilter === type.value}
                    onClick={() => {
                      setTypeFilter(type.value);
                      setPeekIndex(null);
                    }}
                  >
                    {tr(type.en, type.zh)}
                  </button>
                ))}
              </span>
              <span className="tvw-pop-label">{tr('View', '视图')}</span>
              <span className="tvl-pop-group">
                {(
                  [
                    { en: 'All', zh: '全部', view: 'all' as const },
                    { en: 'Unsorted', zh: '未分类', view: 'unsorted' as const },
                  ] satisfies Array<{ en: string; zh: string; view: LibraryScope['view'] }>
                ).map((view) => (
                  <button
                    key={view.en}
                    type="button"
                    className="tvl-choice"
                    aria-pressed={scope.view === view.view}
                    onClick={() => selectScope({ view: view.view, folderId: null })}
                  >
                    {tr(view.en, view.zh)}
                  </button>
                ))}
              </span>
              {folders.length > 0 && (
                <>
                  <span className="tvw-pop-label">{tr('Folder', '文件夹')}</span>
                  <span className="tvl-pop-group">
                    {folders.map(({ folder }) => (
                      <button
                        key={folder.id}
                        type="button"
                        className="tvl-choice"
                        aria-pressed={scope.folderId === folder.id}
                        onClick={() => selectScope({ view: 'folder', folderId: folder.id })}
                      >
                        {folder.name}
                      </button>
                    ))}
                  </span>
                </>
              )}
            </span>
          )}
        </span>
      </div>

      <div className="tvl-bar">
        <div className="tvl-tabs" role="tablist" aria-label={tr('Views', '视图')}>
          {TABS.map((tab) => (
            <button
              key={tab.en}
              type="button"
              role="tab"
              className="tvl-tab"
              aria-selected={scope.view === tab.view}
              onClick={() => selectScope({ view: tab.view, folderId: null })}
            >
              {tr(tab.en, tab.zh)}
            </button>
          ))}
        </div>
        <div className="tvl-active">
          {typeFilter !== null && (
            <button
              type="button"
              className="tvl-active-chip"
              aria-label={tr('Remove type filter', '清除类型筛选')}
              onClick={() => setTypeFilter(null)}
            >
              {tr(
                TYPES.find((type) => type.value === typeFilter)?.en ?? typeFilter,
                TYPES.find((type) => type.value === typeFilter)?.zh ?? typeFilter,
              )}
              <span aria-hidden="true">×</span>
            </button>
          )}
          {scope.folderId !== null && (
            <button
              type="button"
              className="tvl-active-chip"
              aria-label={tr('Remove folder filter', '清除文件夹筛选')}
              onClick={() => selectScope({ view: 'all', folderId: null })}
            >
              {folderNames.get(scope.folderId) ?? tr('Folder', '文件夹')}
              <span aria-hidden="true">×</span>
            </button>
          )}
        </div>
      </div>

      {organiseOpen && (
        <section className="tvl-organise" aria-label={tr('Folders & tags', '文件夹与标签')}>
          <div className="tvl-organise-head">
            <span className="tvw-label">{tr('Folders & tags', '文件夹与标签')}</span>
            <Action label={tr('Done', '完成')} onRun={() => setOrganiseOpen(false)} />
          </div>
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
              if (scope.folderId === folder.id) selectScope({ view: 'all', folderId: null });
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
        </section>
      )}

      <div ref={viewportRef} className="tvl-list" data-testid="library-viewport">
        {failed ? (
          <div className="tvw-empty tvl-state">
            <strong>
              {tr('Your snippets are safe on this Mac.', '你的片段仍安全地保存在本机。')}
            </strong>
            {tr('This view failed to load just now.', '只是此视图刚才加载失败。')}
            <div style={{ marginTop: 10 }}>
              <button type="button" className="tvw-chip" onClick={retry}>
                {tr('Retry', '重试')}
              </button>
            </div>
          </div>
        ) : rowCount === 0 ? (
          <div className="tvw-empty tvl-state">
            {searchRows !== null ? (
              <>
                <strong>{tr('No snippet matches that', '没有匹配的片段')}</strong>
                {tr(
                  'Save what you typed as a new snippet, or clear the filters.',
                  '可以把刚输入的内容保存为新片段,或清除筛选条件。',
                )}
                <div style={{ display: 'flex', gap: 6, marginTop: 10 }}>
                  <button
                    type="button"
                    className="tvw-chip"
                    onClick={() => void navigate('/editor', { state: { draftTitle: query } })}
                  >
                    {tr('Save as snippet', '保存为片段')}
                  </button>
                  <button type="button" className="tvw-chip" onClick={clearFilters}>
                    {tr('Clear filters', '清除筛选')}
                  </button>
                </div>
              </>
            ) : (
              <>
                <strong>
                  {scope.folderId !== null
                    ? tr('This folder is empty', '这个文件夹是空的')
                    : tr('Nothing here yet', '这里还没有内容')}
                </strong>
                {scope.folderId !== null
                  ? tr('Move snippets here from any list.', '可以从任意列表把片段移到这里。')
                  : tr(
                      'Snippets you save will appear here as rows.',
                      '保存的片段会以行的形式显示在这里。',
                    )}
              </>
            )}
          </div>
        ) : (
          <div
            className="tvl-spacer"
            style={{ height: totalHeight + (peekIndex === null ? 0 : PEEK_HEIGHT) }}
          >
            {slots}
            {peeked !== undefined && peekIndex !== null && (
              <div
                className="tvl-peek"
                role="region"
                aria-label={tr('Details', '详情')}
                style={{
                  position: 'absolute',
                  left: 0,
                  right: 0,
                  transform: `translateY(${String((peekIndex + 1) * ROW_HEIGHT)}px)`,
                }}
              >
                <div className="tvl-peek-body">
                  {peeked.body ?? tr('Kept in the Vault.', '保存在保险库中。')}
                </div>
                <dl className="tvl-peek-meta">
                  <div>
                    <dt>{tr('Trigger', '触发词')}</dt>
                    <dd>{peeked.trigger ?? tr('none', '无')}</dd>
                  </div>
                  <div>
                    <dt>{tr('Folder', '文件夹')}</dt>
                    <dd>{folderNames.get(peeked.folderId ?? '') ?? tr('Unsorted', '未分类')}</dd>
                  </div>
                  <div>
                    <dt>{tr('Used', '使用次数')}</dt>
                    <dd>
                      {tr(
                        peeked.usageCount === 1 ? '1 time' : `${String(peeked.usageCount)} times`,
                        `${String(peeked.usageCount)} 次`,
                      )}
                    </dd>
                  </div>
                  <div>
                    <dt>{tr('Version', '版本')}</dt>
                    <dd>{peeked.version}</dd>
                  </div>
                </dl>
                <div className="tvl-peek-actions">
                  <Action
                    label={tr('Edit', '编辑')}
                    cta
                    onRun={() =>
                      void navigate(
                        peeked.securityLevel === 'sensitive' ? '/vault' : `/editor/${peeked.id}`,
                      )
                    }
                  />
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {selected.size > 0 && (
        <div className="tvw-selbar" role="group" aria-label={tr('Selection', '已选片段')}>
          {confirmDelete ? (
            <>
              <span className="tvw-selbar-count">
                {tr(
                  `Delete ${String(selected.size)} snippet${selected.size === 1 ? '' : 's'}?`,
                  `删除 ${String(selected.size)} 个片段?`,
                )}
              </span>
              <Action
                label={tr(
                  `Delete ${String(selected.size)} snippet${selected.size === 1 ? '' : 's'}`,
                  `删除 ${String(selected.size)} 个片段`,
                )}
                onRun={() => runBatch(batchTrashSnippets([...selected]).then(notifyMutation))}
              />
              <Action label={tr('Cancel', '取消')} onRun={() => setConfirmDelete(false)} />
            </>
          ) : (
            <>
              <span className="tvw-selbar-count">
                {tr(`${String(selected.size)} selected`, `已选 ${String(selected.size)} 项`)}
              </span>
              <span className="tvw-menu-wrap">
                <Action
                  label={tr('Move', '移动')}
                  onRun={() => setBatchMenu(batchMenu === 'move' ? 'none' : 'move')}
                />
                {batchMenu === 'move' && (
                  <span className="tvw-menu" role="menu" aria-label={tr('Move', '移动')}>
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => runBatch(batchMoveSnippets([...selected], null))}
                    >
                      {tr('Unsorted', '未分类')}
                    </button>
                    {folders.map(({ folder }) => (
                      <button
                        key={folder.id}
                        type="button"
                        role="menuitem"
                        onClick={() => runBatch(batchMoveSnippets([...selected], folder.id))}
                      >
                        {folder.name}
                      </button>
                    ))}
                  </span>
                )}
              </span>
              <span className="tvw-menu-wrap">
                <Action
                  label={tr('Tag', '标签')}
                  onRun={() => setBatchMenu(batchMenu === 'tag' ? 'none' : 'tag')}
                />
                {batchMenu === 'tag' && (
                  <span className="tvw-menu" role="menu" aria-label={tr('Tag', '标签')}>
                    {tags.length === 0 ? (
                      <button type="button" role="menuitem" onClick={() => setBatchMenu('none')}>
                        {tr('No tags yet', '还没有标签')}
                      </button>
                    ) : (
                      tags.map((tag) => (
                        <button
                          key={tag.id}
                          type="button"
                          role="menuitem"
                          onClick={() => runBatch(batchTagSnippets([...selected], tag.id))}
                        >
                          {tag.name}
                        </button>
                      ))
                    )}
                  </span>
                )}
              </span>
              <Action label={tr('Delete', '删除')} onRun={() => setConfirmDelete(true)} />
              <Action label={tr('Done', '完成')} onRun={clearSelection} />
            </>
          )}
        </div>
      )}
      {context.node}
    </div>
  );
}
