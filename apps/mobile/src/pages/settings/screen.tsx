// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * A pushed settings screen: a text back bar naming the
 * parent screen, then the screen's own title. No tab bar, no chevron glyph,
 * no transition — navigation is a cut.
 *
 * Android's system back dismisses the screen before anything else, matching
 * the back bar exactly.
 */
import { useEffect, type ReactNode } from 'react';
import { pushBackHandler } from '../../platform';
import './settings.css';

interface ScreenProps {
  /** Name of the screen the back bar returns to. */
  parent: string;
  title: string;
  /** One introductory sentence above the first group. */
  intro?: ReactNode;
  onBack: () => void;
  children: ReactNode;
}

export function SettingsScreen({ parent, title, intro, onBack, children }: ScreenProps) {
  useEffect(() => pushBackHandler(onBack), [onBack]);

  return (
    <main className="tv-mset">
      <button type="button" className="tv-mset-back" onClick={onBack}>
        {parent}
      </button>
      <h1 className="tv-mset-title">{title}</h1>
      {intro !== undefined && <p className="tv-mset-intro">{intro}</p>}
      {children}
    </main>
  );
}

/** Loading state for a pushed screen: text lines, never a spinner. */
export function ScreenSkeleton({ parent, onBack }: { parent: string; onBack: () => void }) {
  return (
    <main className="tv-mset" aria-busy="true">
      <button type="button" className="tv-mset-back" onClick={onBack}>
        {parent}
      </button>
      <div className="tv-mset-skeleton" />
      <div className="tv-mset-skeleton" />
      <div className="tv-mset-skeleton" />
    </main>
  );
}
