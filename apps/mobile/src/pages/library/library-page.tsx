// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { countSnippets, listFolderChildren, listSnippetPage, searchLibrary } from '@typvia/shared';
import type { Folder, Snippet } from '@typvia/shared';
import { Caret, SnippetStripSkeleton, useTr } from '@typvia/ui';
import { useEffect, useRef, useState } from 'react';
import { SearchLineInput } from '../../components/search-line';
import { SnippetStripRow } from '../../components/snippet-strip-row';
import './library.css';

const PAGE_SIZE = 100;
const SEARCH_LIMIT = 200;
const SKELETON_STRIPS = 6;

/** Filter vocabulary as drawn: All · Text · Code · Prompt · Template.
 *  Other types stay reachable under All. */
const TYPE_TABS: ReadonlyArray<{ key: string; en: string; zh: string; value: string | null }> = [
  { key: 'all', en: 'All', zh: '全部', value: null },
  { key: 'text', en: 'Text', zh: '文本', value: 'text' },
  { key: 'code', en: 'Code', zh: '代码', value: 'code' },
  { key: 'prompt', en: 'Prompt', zh: '提示词', value: 'prompt' },
  { key: 'template', en: 'Template', zh: '模板', value: 'template' },
];

interface LibraryPageProps {
  /** Row tap opens the snippet detail screen. */
  onOpen: (snippet: Snippet) => void;
}

/**
 * Mobile Library: title with the count in
 * the corner, a quiet search line, underline type tabs, and folder-grouped
 * strips with usage counts. Search replaces the list instantly on every
 * keystroke — no debounce, no transition.
 */
export function LibraryPage({ onOpen }: LibraryPageProps) {
  const tr = useTr();
  const [query, setQuery] = useState('');
  const [tabKey, setTabKey] = useState('all');
  // Browse mode: the paged list for the active type filter; null = loading.
  const [rows, setRows] = useState<Snippet[] | null>(null);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [total, setTotal] = useState(0);
  // Search mode: null while browsing or a keystroke is in flight.
  const [results, setResults] = useState<Snippet[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  // Independent stamps per mode: a keystroke must never orphan the browse
  // list it will fall back to when the query is cleared.
  const browseGeneration = useRef(0);
  const searchGeneration = useRef(0);

  const typeFilter = TYPE_TABS.find((tab) => tab.key === tabKey)?.value ?? null;
  const searching = query.trim() !== '';

  useEffect(() => {
    browseGeneration.current += 1;
    const stamp = browseGeneration.current;
    setRows(null);
    setFailed(false);
    Promise.all([
      countSnippets('all', null, typeFilter),
      listSnippetPage('all', null, typeFilter, PAGE_SIZE, 0),
      listFolderChildren(null),
    ])
      .then(([count, page, roots]) => {
        if (browseGeneration.current !== stamp) return;
        setTotal(count);
        setRows(page);
        setFolders(roots);
      })
      .catch(() => {
        if (browseGeneration.current === stamp) setFailed(true);
      });
  }, [typeFilter, attempt]);

  // Every keystroke replaces the results instantly (previous results stay
  // until the answer lands — no clearing, no transition); the stamp drops
  // stale responses so a slow earlier query never wins.
  useEffect(() => {
    searchGeneration.current += 1;
    const stamp = searchGeneration.current;
    if (query.trim() === '') {
      setResults(null);
      return;
    }
    searchLibrary(query, SEARCH_LIMIT)
      .then((hits) => {
        if (searchGeneration.current === stamp) setResults(hits);
      })
      .catch(() => {
        if (searchGeneration.current === stamp) setFailed(true);
      });
  }, [query, attempt]);

  const loadMore = () => {
    if (rows === null) return;
    const stamp = browseGeneration.current;
    listSnippetPage('all', null, typeFilter, PAGE_SIZE, rows.length)
      .then((page) => {
        if (browseGeneration.current !== stamp) return;
        setRows((previous) => (previous === null ? page : [...previous, ...page]));
      })
      .catch(() => {
        if (browseGeneration.current === stamp) setFailed(true);
      });
  };

  const visible = searching ? results : rows;
  const activeTab = TYPE_TABS.find((tab) => tab.key === tabKey);
  const activeTabLabel =
    activeTab === undefined ? tr('All', '全部') : tr(activeTab.en, activeTab.zh);

  const renderStrips = (list: Snippet[]) => (
    <ul className="tv-mobile-strips tv-strip-list">
      {list.map((snippet) => (
        <li key={snippet.id}>
          <SnippetStripRow
            snippet={snippet}
            onOpen={onOpen}
            meta="none"
            trailing={
              snippet.usageCount > 0 ? (
                <span className="tv-mlib-uses">{snippet.usageCount}×</span>
              ) : undefined
            }
          />
        </li>
      ))}
    </ul>
  );

  // Folder-grouped sections (browse mode): folders in their own order, then
  // the unfiled remainder. Searching flattens to one list.
  const grouped =
    visible === null || searching
      ? []
      : [
          ...folders
            .map((folder) => ({
              key: folder.id,
              name: folder.name,
              list: visible.filter((s) => s.folderId === folder.id),
            }))
            .filter((group) => group.list.length > 0),
          {
            key: 'unfiled',
            name: tr('Unfiled', '未分组'),
            list: visible.filter(
              (s) => s.folderId === null || !folders.some((f) => f.id === s.folderId),
            ),
          },
        ].filter((group) => group.list.length > 0);

  return (
    <main className="tv-mobile-page tv-mlib">
      <header className="tv-mlib-head">
        <h1 className="tv-mlib-title">{tr('Library', '资料库')}</h1>
        <span className="tv-mlib-count" role="status" aria-live="polite">
          {searching ? (results === null ? '' : results.length) : total.toLocaleString('en-US')}
        </span>
      </header>

      <div className="tv-mlib-search">
        <SearchLineInput
          value={query}
          onChange={setQuery}
          placeholder={tr('Search library', '搜索资料库')}
          label={tr('Search library', '搜索资料库')}
          quiet
        />
      </div>

      {/* searchLibrary carries no snippet-type parameter and type filtering
          is a core-side rule, so the tabs are inert while a query is active
          instead of pretending to narrow the results. */}
      <div role="group" aria-label={tr('Filter by type', '按类型筛选')} className="tv-mlib-tabs">
        {TYPE_TABS.map((tab) => {
          const active = tab.key === tabKey;
          return (
            <button
              key={tab.key}
              type="button"
              aria-pressed={active}
              aria-disabled={searching || undefined}
              className={active ? 'tv-mlib-tab is-active' : 'tv-mlib-tab'}
              onClick={() => {
                if (!searching) setTabKey(tab.key);
              }}
            >
              <span>{tr(tab.en, tab.zh)}</span>
              {active && <span className="tv-mlib-tab-bar" />}
            </button>
          );
        })}
      </div>

      {failed ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-title">
            {tr('Your snippets are safe on this device.', '你的片段在这台设备上安然无恙。')}
          </div>
          <div className="tv-mobile-state-text">
            {tr('This view failed to load just now.', '当前视图刚才未能加载。')}
          </div>
          <button
            type="button"
            className="tv-mobile-state-action"
            onClick={() => {
              setFailed(false);
              setAttempt((n) => n + 1);
            }}
          >
            {tr('Retry', '重试')}
          </button>
        </div>
      ) : visible === null ? (
        <div className="tv-mobile-strips">
          {Array.from({ length: SKELETON_STRIPS }, (_, index) => (
            <SnippetStripSkeleton key={index} />
          ))}
        </div>
      ) : searching && visible.length === 0 ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-figure">
            <span className="tv-mobile-state-slot" />
            <Caret height={16} />
          </div>
          <div className="tv-mobile-state-title">
            {tr(`No snippet matches “${query}”`, `没有片段匹配“${query}”`)}
          </div>
          <div className="tv-mobile-state-text">
            {tr('Try fewer letters, or clear the search.', '试试更少的字母，或清除搜索。')}
          </div>
          <button
            type="button"
            className="tv-mobile-state-action"
            onClick={() => {
              setQuery('');
            }}
          >
            {tr('Clear search', '清除搜索')}
          </button>
        </div>
      ) : visible.length === 0 && typeFilter !== null ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-figure">
            <span className="tv-mobile-state-slot" />
            <Caret height={16} />
          </div>
          <div className="tv-mobile-state-title">
            {tr(`No ${activeTabLabel} snippets`, `没有${activeTabLabel}片段`)}
          </div>
          <div className="tv-mobile-state-text">
            {tr('Nothing of this type is saved yet.', '还没有保存这种类型的片段。')}
          </div>
          <button
            type="button"
            className="tv-mobile-state-action"
            onClick={() => {
              setTabKey('all');
            }}
          >
            {tr('Show all', '显示全部')}
          </button>
        </div>
      ) : visible.length === 0 ? (
        <div className="tv-mobile-state">
          <div className="tv-mobile-state-figure">
            <span className="tv-mobile-state-slot" />
            <Caret height={16} />
          </div>
          <div className="tv-mobile-state-title">{tr('Your library is empty', '资料库是空的')}</div>
          <div className="tv-mobile-state-text">
            {tr(
              'Save a snippet with the centre button — it appears here at once.',
              '用中间的新建按钮保存片段，它会立刻出现在这里。',
            )}
          </div>
        </div>
      ) : searching ? (
        renderStrips(visible)
      ) : (
        <>
          {grouped.map((group) => (
            <section key={group.key} aria-label={group.name} className="tv-mlib-group">
              <div className="tv-mobile-label">{group.name}</div>
              {renderStrips(group.list)}
            </section>
          ))}
          {rows !== null && rows.length < total && (
            <button type="button" className="tv-mobile-state-action" onClick={loadMore}>
              {tr(
                `Load more · ${String(total - rows.length)} remaining`,
                `加载更多 · 还剩 ${String(total - rows.length)} 条`,
              )}
            </button>
          )}
        </>
      )}
    </main>
  );
}
