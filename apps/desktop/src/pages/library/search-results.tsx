// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Snippet } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import type { MouseEvent, PointerEvent } from 'react';
import { GroupTitle, KeyCap, TextAction } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';
import { useVariables } from './library-data';
import { triggerChars, type CollectionKey, type SearchGroups } from './library-model';
import { Highlight, SnippetRow, type RowFacts } from './snippet-row';

export interface CollectionHit {
  key: CollectionKey;
  name: string;
  count: number;
}

export const resultId = (index: number) => `tvl-result-${String(index)}`;

/** Nothing matched: no "no results", straight to the ways on, each with its number. */
function NoMatch({
  total,
  trashCount,
  onCreate,
  onOpenTrash,
}: {
  total: number;
  trashCount: number;
  onCreate: () => void;
  onOpenTrash: () => void;
}) {
  const tr = useTr();
  const n = total.toLocaleString('en-US');
  const inTrash = String(trashCount);
  return (
    <div className="tvl-nomatch">
      <div className="tvl-nomatch-line">
        <Mascot state="confused" size={40} />
        <p className="tvl-nomatch-title">
          {tr(`None of the ${n} says this.`, `${n} 条里没有这一句。`)}
        </p>
      </div>
      <div className="tvl-nomatch-ways">
        <TextAction primary onClick={onCreate}>
          {tr('Make a snippet from these words', '就用这几个字新建一条')}
        </TextAction>
        {trashCount > 0 && (
          <TextAction onClick={onOpenTrash}>
            {tr(`Look in the trash · ${inTrash} there`, `去回收站找 · 那里还有 ${inTrash} 条`)}
          </TextAction>
        )}
      </div>
      <div className="tvl-nomatch-foot">
        <KeyCap>⌘⏎</KeyCap>
        <span>{tr('Create it now', '直接新建')}</span>
        <span className="tvl-grow" />
        <span className="tvl-nomatch-esc">{tr('esc to clear', 'esc 收起')}</span>
      </div>
    </div>
  );
}

/**
 * Results in three groups — the text, the trigger, the collection name — each
 * titled with its count. The picked row drives the detail pane like a list pick.
 */
export function SearchResults({
  query,
  groups,
  collections,
  pick,
  onPick,
  onRowMenu,
  onRowPress,
  onOnlyCollection,
  factsFor,
  total,
  trashCount,
  onCreate,
  onOpenTrash,
}: {
  query: string;
  groups: SearchGroups;
  collections: readonly CollectionHit[];
  pick: number;
  onPick: (index: number) => void;
  onRowMenu: (event: MouseEvent, index: number, snippet: Snippet, hasVariables: boolean) => void;
  onRowPress: (event: PointerEvent, snippet: Snippet) => void;
  onOnlyCollection: (key: CollectionKey) => void;
  factsFor: (snippet: Snippet) => Omit<RowFacts, 'variables'>;
  total: number;
  trashCount: number;
  onCreate: () => void;
  onOpenTrash: () => void;
}) {
  const tr = useTr();
  const rows = [...groups.body, ...groups.trigger];
  const variablesOf = useVariables(rows);
  const triggerWidth = triggerChars(rows.map((snippet) => snippet.trigger));

  if (rows.length === 0 && collections.length === 0) {
    return (
      <NoMatch
        total={total}
        trashCount={trashCount}
        onCreate={onCreate}
        onOpenTrash={onOpenTrash}
      />
    );
  }

  const row = (snippet: Snippet, index: number) => (
    <SnippetRow
      key={snippet.id}
      id={resultId(index)}
      snippet={snippet}
      query={query}
      facts={{ ...factsFor(snippet), variables: variablesOf(snippet)?.length ?? 0 }}
      picked={index === pick}
      triggerWidth={triggerWidth}
      onPick={() => onPick(index)}
      onMenu={(event) => onRowMenu(event, index, snippet, (variablesOf(snippet)?.length ?? 0) > 0)}
      onPress={(event) => onRowPress(event, snippet)}
    />
  );
  const inText = String(groups.body.length);
  const inTrigger = String(groups.trigger.length);
  const inNames = String(collections.length);

  return (
    <div className="tvl-results">
      {rows.length > 0 && (
        <div
          role="listbox"
          tabIndex={0}
          aria-label={tr('Results', '搜索结果')}
          aria-activedescendant={rows[pick] === undefined ? undefined : resultId(pick)}
        >
          {groups.body.length > 0 && (
            <section role="group" aria-label={tr('In the text', '正文里')} className="tvl-group">
              <GroupTitle>{tr(`In the text · ${inText}`, `正文里 · ${inText} 条`)}</GroupTitle>
              <div className="tvl-group-rows">
                {groups.body.map((snippet, i) => row(snippet, i))}
              </div>
            </section>
          )}
          {groups.trigger.length > 0 && (
            <section
              role="group"
              aria-label={tr('In the trigger', '触发词里')}
              className="tvl-group"
            >
              <GroupTitle>
                {tr(`In the trigger · ${inTrigger}`, `触发词里 · ${inTrigger} 条`)}
              </GroupTitle>
              <div className="tvl-group-rows">
                {groups.trigger.map((snippet, i) => row(snippet, groups.body.length + i))}
              </div>
            </section>
          )}
        </div>
      )}
      {collections.length > 0 && (
        <section className="tvl-group">
          <GroupTitle>
            {tr(`In a collection name · ${inNames}`, `集合名里 · ${inNames} 条`)}
          </GroupTitle>
          {collections.map((hit) => (
            <div key={hit.key} className="tvl-collection-hit">
              <span className="tvl-collection-hit-name">
                <Highlight text={hit.name} query={query} />
              </span>
              <span className="tvl-collection-hit-count">
                {tr(`${String(hit.count)}`, `${String(hit.count)} 条`)}
              </span>
              <TextAction onClick={() => onOnlyCollection(hit.key)}>
                {tr('Only this collection', '只看这个集合')}
              </TextAction>
            </div>
          ))}
        </section>
      )}
      <p className="tvl-results-note">
        {tr(
          'Search reads the text, the trigger and the collection name at once, in three groups. No fuzzy score — most recently used first.',
          '搜索同时看正文、触发词与集合名,分三组给。不做模糊匹配的分数排序 —— 按最近用过排。',
        )}
      </p>
    </div>
  );
}
