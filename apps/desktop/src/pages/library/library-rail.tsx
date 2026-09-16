// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useTr } from '@typvia/ui';
import type { ReactNode, RefObject } from 'react';
import { Choice, type ChoiceOption } from '../../paper/choice';
import { GroupTitle } from '../../paper/kit';
import { Mascot } from '../../paper/mascot';
import type { CollectionKey, SyncTone } from './library-model';

/**
 * The collections column: brand, the search line, collections as a stacked
 * trace choice, the vault's door, and the sync line at the foot. No icon
 * cells and no selected block — the chosen collection is ink plus a line.
 * Arranging the collections happens in this same column, not in a dialog.
 */
export function LibraryRail({
  collections,
  collection,
  onCollection,
  marked,
  managing,
  onToggleManage,
  manage,
  query,
  onQuery,
  searchRef,
  vaultLabel,
  onOpenVault,
  sync,
}: {
  collections: readonly ChoiceOption<CollectionKey>[];
  collection: CollectionKey;
  onCollection: (key: CollectionKey) => void;
  /** Where a dragged snippet would land. */
  marked: CollectionKey | null;
  managing: boolean;
  onToggleManage: () => void;
  /** The arranging view, shown in place of the choice while managing. */
  manage: ReactNode;
  query: string;
  onQuery: (query: string) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  vaultLabel: string;
  onOpenVault: () => void;
  sync: { text: string; tone: SyncTone } | null;
}) {
  const tr = useTr();
  return (
    <aside className="tvl-rail" aria-label={tr('Collections', '集合')}>
      <div className="tvl-brand">
        <Mascot size={26} />
        <span className="tvl-brand-name">TYPVIA</span>
      </div>

      <label className="tvl-search">
        <svg width="13" height="13" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <circle cx="6" cy="6" r="4.6" stroke="currentColor" strokeWidth="1.3" />
          <path d="M9.6 9.6L13 13" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
        <input
          ref={searchRef}
          type="text"
          value={query}
          aria-label={tr('Search snippets', '搜索片段')}
          placeholder={tr('Search snippets', '搜索片段')}
          onChange={(event) => onQuery(event.target.value)}
        />
        <span aria-hidden="true" className="tvl-search-key">
          ⌘F
        </span>
      </label>

      <div className="tvl-rail-group">
        <GroupTitle
          trailing={
            <button type="button" className="tvl-group-toggle" onClick={onToggleManage}>
              {managing ? tr('Done', '完成') : tr('Manage', '管理')}
            </button>
          }
        >
          {managing ? tr('Collections · arranging', '集合 · 管理态') : tr('Collections', '集合')}
        </GroupTitle>
      </div>
      {managing ? (
        manage
      ) : (
        <div className="tvl-collections">
          <Choice
            stacked
            label={tr('Collections', '集合')}
            options={collections}
            value={collection}
            onChange={onCollection}
            marked={marked}
          />
        </div>
      )}

      <div className="tvl-rail-group is-vault">
        <GroupTitle>{tr('Vault', '保险库')}</GroupTitle>
      </div>
      <button type="button" className="tvl-vault" onClick={onOpenVault}>
        <span>{vaultLabel}</span>
        <svg width="11" height="13" viewBox="0 0 11 13" fill="none" aria-hidden="true">
          <path
            d="M2.2 5.4V3.6a3.3 3.3 0 016.6 0v1.8"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
          />
          <rect
            x="1"
            y="5.4"
            width="9"
            height="6.8"
            rx="1.6"
            stroke="currentColor"
            strokeWidth="1.3"
          />
        </svg>
      </button>

      <div className="tvl-rail-foot">
        {sync !== null && (
          <>
            {sync.tone === 'paused' || sync.tone === 'warn' ? (
              <Mascot state="sleep" size={16} />
            ) : (
              <span aria-hidden="true" className={`tvl-dot is-${sync.tone}`} />
            )}
            <span>{sync.text}</span>
          </>
        )}
      </div>
    </aside>
  );
}
