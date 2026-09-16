// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { getCurrentWindow } from '@tauri-apps/api/window';
import { summonShortcut } from '@typvia/shared';
import { useTr } from '@typvia/ui';
import { useEffect, useState } from 'react';
import { KeyCap } from '../paper/kit';
import { Mascot } from '../paper/mascot';
import { BackAction, PlaceTabs } from './title-nav';
import './window-bar.css';

/**
 * The title bar where the system draws none of its own: the mark, the four
 * places and the way back, the summon key, and the three window buttons, each
 * 46 wide. Only the close button ever takes a solid fill, and only while
 * hovered.
 */
export function WindowBar() {
  const tr = useTr();
  // The host reports what the system actually granted; when it granted
  // nothing there is no key to name, so the bar simply does not offer one.
  const [summon, setSummon] = useState<string | null>(null);
  useEffect(() => {
    summonShortcut()
      .then(setSummon)
      .catch(() => setSummon(null));
  }, []);

  return (
    <header className="tpi tvwin" data-tauri-drag-region="">
      <span className="tvwin-brand" data-tauri-drag-region="">
        <Mascot state="idle" size={21} />
        <span className="tvwin-mark">TYPVIA</span>
      </span>
      <span className="tvwin-menus">
        <PlaceTabs />
        <BackAction />
      </span>
      <span className="tvwin-grow" data-tauri-drag-region="" />
      {summon !== null && (
        <span className="tvwin-summon">
          <span>{tr('Summon', '呼出')}</span>
          <KeyCap>{summon}</KeyCap>
        </span>
      )}
      <div className="tvwin-controls">
        <button
          type="button"
          aria-label={tr('Minimize', '最小化')}
          onClick={() => void getCurrentWindow().minimize()}
        >
          <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
            <path d="M1 5.5h9" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>
        <button
          type="button"
          aria-label={tr('Maximize', '最大化')}
          onClick={() => void getCurrentWindow().toggleMaximize()}
        >
          <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
            <rect
              x="1.5"
              y="1.5"
              width="8"
              height="8"
              stroke="currentColor"
              strokeWidth="1.1"
              fill="none"
            />
          </svg>
        </button>
        <button
          type="button"
          className="is-close"
          aria-label={tr('Close', '关闭')}
          onClick={() => void getCurrentWindow().close()}
        >
          <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
            <path d="M1.5 1.5l8 8M9.5 1.5l-8 8" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>
      </div>
    </header>
  );
}
