// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { ReactNode } from 'react';
import { useTr } from '../i18n';
import './tab-bar.css';

export interface TabBarItem {
  key: string;
  label: string;
  /** 20px line icon drawn above the label; colour follows the tab state. */
  icon?: ReactNode;
  /** Rendered but inert — for surfaces that have not shipped on mobile yet. */
  disabled?: boolean;
}

interface TabBarProps {
  items: readonly TabBarItem[];
  activeKey: string;
  onSelect: (key: string) => void;
  /**
   * When present, the centre slot holds the dark rounded create square
   * (36px, r12, ink fill with a paper cross — never a coloured FAB). Hosts
   * without a create surface omit it and the bar collapses to the tabs.
   */
  onCreate?: (() => void) | undefined;
}

/**
 * Mobile bottom navigation (design: typvia_mobile_export): one thin layer —
 * a hairline on paper, four line icons with labels, the create square in the
 * middle. Active is expressed by darkening (ink + weight), never by fills. A
 * disabled tab stays visible and announced (aria-disabled) instead of
 * pretending its page exists.
 */
export function TabBar({ items, activeKey, onSelect, onCreate }: TabBarProps) {
  const tr = useTr();
  const renderTab = (item: TabBarItem) => {
    const active = item.key === activeKey;
    return (
      <button
        key={item.key}
        type="button"
        aria-current={active ? 'page' : undefined}
        aria-disabled={item.disabled === true || undefined}
        className={active ? 'tv-tab is-active' : 'tv-tab'}
        onClick={() => {
          if (item.disabled !== true) onSelect(item.key);
        }}
      >
        {item.icon !== undefined && <span className="tv-tab-icon">{item.icon}</span>}
        <span className="tv-tab-label">{item.label}</span>
      </button>
    );
  };

  return (
    <nav aria-label={tr('Main', '主导航')} className="tv-tab-bar">
      {items.slice(0, 2).map(renderTab)}
      {onCreate !== undefined && (
        <div className="tv-tab-create-slot">
          <button
            type="button"
            className="tv-tab-create"
            aria-label={tr('New snippet', '新建片段')}
            onClick={onCreate}
          />
        </div>
      )}
      {items.slice(2).map(renderTab)}
    </nav>
  );
}
