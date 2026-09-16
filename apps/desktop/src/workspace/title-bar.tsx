// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { createContext, useContext, useEffect, useRef, type ReactNode } from 'react';

/** What a page puts at the title bar's right end in place of the summon key. */
export interface TitleBarContent {
  trailing: ReactNode;
}

export const TitleBarContext = createContext<(content: TitleBarContent | null) => void>(
  () => undefined,
);

/**
 * Lets the page on screen speak in the title bar while it is mounted. `key`
 * names the content, so the bar is rewritten only when what it says changes —
 * the elements themselves are new objects on every render.
 */
export function useTitleBar(content: TitleBarContent, key: string): void {
  const set = useContext(TitleBarContext);
  const latest = useRef(content);
  latest.current = content;
  useEffect(() => {
    set(latest.current);
  }, [set, key]);
  useEffect(() => () => set(null), [set]);
}
