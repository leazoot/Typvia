// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { libraryCounts, searchLibraryDeep, searchSnippets, vaultList } from '@typvia/shared';
import type { Folder, LibraryCounts, ListOrder, Snippet } from '@typvia/shared';
import { useLocale, useTr } from '@typvia/ui';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';
import { useNavigate } from 'react-router';
import { editsText } from '../../backspace-guard';
import { usePaperMenu, type MenuPoint } from '../../paper/menu';
import { useVault } from '../../vault/vault-context';
import { useCommand } from '../../workspace/commands';
import { announceLibraryChange, useLibraryChange } from '../../workspace/library-events';
import { fetchFolders, useSyncPicture, useVariables } from './library-data';
import { CollectionsManage } from './collections-manage';
import {
  collectionNameOf,
  firstLine,
  groupResults,
  scopeFor,
  syncLine,
  usedLabel,
  type CollectionKey,
  type SearchGroups,
} from './library-model';
import { LibraryRail } from './library-rail';
import { ListState } from './list-states';
import { rowMenu } from './row-menu';
import { resultId, SearchResults } from './search-results';
import { SnippetDetail } from './snippet-detail';
import { ListHead, rowId, SnippetList } from './snippet-list';
import { usePointerDrag } from './use-pointer-drag';
import { useRowActions } from './use-row-actions';
import { useSnippetPages } from './use-snippet-pages';
import './library.css';

const SEARCH_LIMIT = 500;
const count = (value: number) => value.toLocaleString('en-US');

/**
 * Where a snippet dropped on a collection goes: a folder id, '' for Unsorted,
 * or null when the drop moves nothing ("All snippets", or where it already is).
 */
function dropFolderId(value: string, snippet: Snippet): string | null {
  const folderId =
    value === 'unsorted' ? '' : value.startsWith('folder:') ? value.slice('folder:'.length) : null;
  return folderId === null || folderId === (snippet.folderId ?? '') ? null : folderId;
}

/**
 * The main window: collections, the list, and the picked snippet in full.
 * Choosing a collection changes the list, the title, the count and the detail
 * at once — click, write, re-render, with no in-between state.
 */
export function LibraryPage() {
  const tr = useTr();
  const locale = useLocale();
  const navigate = useNavigate();
  const vault = useVault();
  const sync = useSyncPicture();

  const [collection, setCollection] = useState<CollectionKey>('all');
  const [order, setOrder] = useState<ListOrder>('recent');
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchGroups | null>(null);
  const [pick, setPick] = useState(0);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [vaultCount, setVaultCount] = useState<number | null>(null);
  const [searchRound, setSearchRound] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const searchGeneration = useRef(0);

  const pages = useSnippetPages(scopeFor(collection), order);
  const menu = usePaperMenu(tr('Snippet actions', '片段操作'));
  const now = Date.now();

  const refreshMeta = useCallback(() => {
    libraryCounts()
      .then(setCounts)
      .catch(() => {
        // The list column reports load failures; the rail just stays uncounted.
      });
    fetchFolders()
      .then(setFolders)
      .catch(() => setFolders([]));
    vaultList(SEARCH_LIMIT, 0)
      .then((rows) => setVaultCount(rows.length))
      .catch(() => setVaultCount(null));
  }, []);
  useEffect(refreshMeta, [refreshMeta]);

  // Whoever changed the library — this page, the undo note, the menu bar —
  // everything counted here is read again.
  const { retry } = pages;
  useLibraryChange(() => {
    retry();
    refreshMeta();
    setSearchRound((n) => n + 1);
  });

  const actions = useRowActions(() => {
    announceLibraryChange();
    sync.refresh();
  });

  const [managing, setManaging] = useState(false);
  const [fillingId, setFillingId] = useState<string | null>(null);
  const [creatingCollection, setCreatingCollection] = useState(false);
  useCommand('collection.new', () => {
    setManaging(true);
    setCreatingCollection(true);
  });
  const snippetDrag = usePointerDrag<Snippet>({
    targets: '.tvl-collections [data-value]',
    onDrop: (snippet, spot) => {
      const folderId = dropFolderId(spot.value, snippet);
      if (folderId !== null) actions.moveTo(snippet, folderId);
    },
  });

  // Every keystroke replaces the results — no debounce, no transition.
  useEffect(() => {
    searchGeneration.current += 1;
    const generation = searchGeneration.current;
    if (query.trim() === '') {
      setResults(null);
      return;
    }
    Promise.all([searchLibraryDeep(query, SEARCH_LIMIT), searchSnippets(query, SEARCH_LIMIT, 0)])
      .then(([rows, hits]) => {
        if (searchGeneration.current === generation) setResults(groupResults(rows, hits));
      })
      .catch(() => {
        if (searchGeneration.current === generation) setResults({ body: [], trigger: [] });
      });
  }, [query, searchRound]);

  useEffect(() => setPick(0), [collection, order, query]);

  // A collection deleted elsewhere must not leave the list pointing at nothing.
  useEffect(() => {
    if (collection.startsWith('folder:') && !folders.some((f) => `folder:${f.id}` === collection)) {
      if (counts !== null) setCollection('all');
    }
  }, [collection, folders, counts]);

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);

  const folderNames = useMemo(() => new Map(folders.map((f) => [f.id, f.name])), [folders]);
  const factsFor = (snippet: Snippet) => ({
    collection: collectionNameOf(snippet, folderNames, tr),
    used: usedLabel(snippet, now, locale),
  });

  const searchRows = results === null ? null : [...results.body, ...results.trigger];
  const rowCount = searchRows === null ? pages.total : searchRows.length;
  const picked = searchRows === null ? pages.rowAt(pick) : searchRows[pick];
  const pickedVariables = useVariables(picked === undefined ? [] : [picked]);

  const collectionLabel = (key: CollectionKey): string =>
    key === 'all'
      ? tr('All snippets', '全部片段')
      : key === 'unsorted'
        ? tr('Unsorted', '待整理')
        : (folderNames.get(key.slice('folder:'.length)) ?? '');
  const folderCounts = new Map(counts?.folders.map((f) => [f.folderId, f.count]));
  const collections = [
    { value: 'all' as const, label: collectionLabel('all'), trailing: count(counts?.total ?? 0) },
    ...folders.map((folder) => ({
      value: `folder:${folder.id}` as const,
      label: folder.name,
      trailing: count(folderCounts.get(folder.id) ?? 0),
    })),
    {
      value: 'unsorted' as const,
      label: collectionLabel('unsorted'),
      trailing: count(counts?.unsorted ?? 0),
    },
  ];
  const dragging = snippetDrag.drag;
  const marked =
    dragging?.spot == null || dropFolderId(dragging.spot.value, dragging.item) === null
      ? null
      : (collections.find((option) => option.value === dragging.spot?.value)?.value ?? null);
  const railCollections = collections.map((option) =>
    option.value === marked
      ? {
          ...option,
          trailing: (
            <>
              <span className="tvl-drop-note">{tr('Drop here · +1', '放到这里 · +1')}</span>
              {option.trailing}
            </>
          ),
        }
      : option,
  );

  const newSnippet = (draftTitle?: string) =>
    void navigate('/editor', draftTitle === undefined ? undefined : { state: { draftTitle } });

  const openRowMenu = (at: MenuPoint, index: number, snippet: Snippet, hasVariables: boolean) => {
    setPick(index);
    menu.open(
      at,
      rowMenu({
        snippet,
        hasVariables,
        folders,
        vault: vault.status,
        tr,
        actions: {
          insert: () => actions.insert(snippet),
          fill: () => {
            setFillingId(snippet.id);
            setDrawerOpen(true);
          },
          copy: () => actions.copy(snippet),
          edit: () => void navigate(`/editor/${snippet.id}`),
          editTrigger: () =>
            void navigate(`/editor/${snippet.id}`, { state: { focus: 'trigger' } }),
          moveTo: (folderId) => actions.moveTo(snippet, folderId),
          toVault: () => actions.toVault(snippet),
          remove: () => actions.remove(snippet),
        },
      }),
    );
  };
  const onRowMenu = (event: MouseEvent, index: number, snippet: Snippet, hasVariables: boolean) => {
    event.preventDefault();
    openRowMenu({ x: event.clientX, y: event.clientY }, index, snippet, hasVariables);
  };

  useCommand('snippet.edit', () => picked !== undefined && void navigate(`/editor/${picked.id}`));
  useCommand('snippet.copy-plain', () => picked !== undefined && actions.copy(picked));
  useCommand('snippet.to-vault', () => picked !== undefined && actions.toVault(picked));
  useCommand(
    'snippet.move',
    (folderId) => picked !== undefined && actions.moveTo(picked, folderId),
  );
  useCommand('snippet.delete', () => picked !== undefined && actions.remove(picked));

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      setPick((current) => Math.min(Math.max(current + step, 0), Math.max(rowCount - 1, 0)));
      return;
    }
    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && searchRows?.length === 0) {
      event.preventDefault();
      newSnippet(query);
      return;
    }
    if (picked !== undefined && event.key === 'Enter' && event.altKey) {
      event.preventDefault();
      actions.copy(picked);
      return;
    }
    if (picked !== undefined && event.key === 'Enter') {
      event.preventDefault();
      if ((pickedVariables(picked)?.length ?? 0) > 0) {
        setFillingId(picked.id);
        setDrawerOpen(true);
      } else {
        actions.insert(picked);
      }
      return;
    }
    if (
      picked !== undefined &&
      event.key === 'Backspace' &&
      (event.metaKey || event.ctrlKey) &&
      !editsText(event.target)
    ) {
      event.preventDefault();
      actions.remove(picked);
      return;
    }
    if (
      picked !== undefined &&
      (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))
    ) {
      event.preventDefault();
      const row = document.getElementById(searchRows === null ? rowId(pick) : resultId(pick));
      const rect = row?.getBoundingClientRect();
      openRowMenu(
        { x: (rect?.left ?? 0) + 96, y: rect?.bottom ?? 0 },
        pick,
        picked,
        (pickedVariables(picked)?.length ?? 0) > 0,
      );
      return;
    }
    if (event.key === 'Escape') {
      if (drawerOpen) setDrawerOpen(false);
      else if (query !== '') setQuery('');
    }
  };

  const syncState = syncLine(sync.status, now, locale);
  const pickedCollection = picked === undefined ? '' : collectionNameOf(picked, folderNames, tr);
  const detailSync =
    syncState !== null &&
    syncState.tone === 'ok' &&
    sync.devices > 1 &&
    sync.status?.lastSyncAt != null
      ? tr(
          `${String(sync.devices)} devices agree · ${syncState.text.split(' · ')[1] ?? ''}`,
          `${String(sync.devices)} 台设备一致 · ${syncState.text.split(' · ')[1] ?? ''}`,
        )
      : (syncState?.text ?? '—');
  const vaultLabel =
    vault.status === null
      ? tr('Vault', '保险库')
      : !vault.status.initialized
        ? tr('Not set up yet', '还没建起来')
        : `${vault.status.unlocked ? tr('Unlocked', '已解锁') : tr('Locked', '已锁')}${
            vaultCount === null ? '' : tr(` · ${String(vaultCount)}`, ` · ${String(vaultCount)} 条`)
          }`;

  const listState = pages.failed
    ? 'failed'
    : !pages.ready
      ? 'loading'
      : pages.total === 0
        ? collection === 'all'
          ? 'empty-library'
          : 'empty-collection'
        : null;
  const q = query.trim().toLowerCase();
  const onListPick = (index: number) => {
    setPick(index);
    setDrawerOpen(true);
  };

  return (
    <div className="tpi tvl" onKeyDown={onKeyDown}>
      <LibraryRail
        collections={railCollections}
        collection={collection}
        onCollection={(key) => {
          setCollection(key);
          setQuery('');
        }}
        marked={marked}
        managing={managing}
        onToggleManage={() => {
          setManaging((on) => !on);
          setCreatingCollection(false);
        }}
        manage={
          <CollectionsManage
            folders={folders}
            countOf={(folderId) => folderCounts.get(folderId) ?? 0}
            unsortedCount={counts?.unsorted ?? 0}
            creating={creatingCollection}
            onCreate={() => setCreatingCollection(true)}
            onCreateEnd={() => setCreatingCollection(false)}
          />
        }
        query={query}
        onQuery={setQuery}
        searchRef={searchRef}
        vaultLabel={vaultLabel}
        onOpenVault={() => void navigate('/vault')}
        sync={syncState}
      />

      <section className="tvl-column" aria-label={tr('Snippets', '片段')}>
        <ListHead
          title={searchRows === null ? collectionLabel(collection) : tr('Search', '搜索')}
          count={
            searchRows === null
              ? tr(count(pages.total), `${count(pages.total)} 条`)
              : tr(
                  `${String(searchRows.length)} · all collections`,
                  `${String(searchRows.length)} 条 · 全部集合`,
                )
          }
          order={searchRows === null ? order : null}
          onOrder={setOrder}
          onNew={() => newSnippet()}
        />
        {results !== null ? (
          <SearchResults
            query={query}
            groups={results}
            collections={folders
              .filter((folder) => folder.name.toLowerCase().includes(q))
              .map((folder) => ({
                key: `folder:${folder.id}` as const,
                name: folder.name,
                count: folderCounts.get(folder.id) ?? 0,
              }))}
            pick={pick}
            onPick={onListPick}
            onRowMenu={onRowMenu}
            onRowPress={snippetDrag.start}
            onOnlyCollection={(key) => {
              setCollection(key);
              setQuery('');
            }}
            factsFor={factsFor}
            total={counts?.total ?? 0}
            trashCount={counts?.trash ?? 0}
            onCreate={() => newSnippet(query)}
            onOpenTrash={() => void navigate('/trash')}
          />
        ) : listState !== null ? (
          <ListState kind={listState} onNew={() => newSnippet()} onRetry={pages.retry} />
        ) : (
          <SnippetList
            label={collectionLabel(collection)}
            pages={pages}
            pick={pick}
            onPick={onListPick}
            onRowMenu={onRowMenu}
            onRowPress={snippetDrag.start}
            factsFor={factsFor}
          />
        )}
      </section>

      <SnippetDetail
        key={picked?.id}
        snippet={picked}
        collection={pickedCollection}
        variables={picked === undefined ? undefined : pickedVariables(picked)}
        used={picked === undefined ? '' : usedLabel(picked, now, locale)}
        sync={detailSync}
        total={counts?.total ?? 0}
        failure={
          actions.failure !== null && actions.failure.id === picked?.id
            ? actions.failure.kind
            : null
        }
        open={drawerOpen}
        filling={picked !== undefined && fillingId === picked.id}
        onInsert={() => picked !== undefined && actions.insert(picked)}
        onFill={() => picked !== undefined && setFillingId(picked.id)}
        onFillCancel={() => setFillingId(null)}
        onInsertFilled={(values) => {
          if (picked === undefined) return;
          setFillingId(null);
          actions.insertTemplate(picked, values);
        }}
        onCopy={() => picked !== undefined && actions.copy(picked)}
        onEdit={() => picked !== undefined && void navigate(`/editor/${picked.id}`)}
        onClose={() => setDrawerOpen(false)}
      />
      {menu.node}
      {dragging !== null && (
        <div
          aria-hidden="true"
          className="tvl-ghost"
          style={{ left: dragging.x + 14, top: dragging.y + 10 }}
        >
          {firstLine(dragging.item)}
        </div>
      )}
    </div>
  );
}
