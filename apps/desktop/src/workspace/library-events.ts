// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { useEffect, useRef } from 'react';

const LIBRARY_CHANGED = 'typvia:library-changed';

/**
 * Says that snippets, collections or the trash changed, so every surface that
 * shows a count of them reads it again — the page that made the change is not
 * the only one showing it (the menu bar's status lines are another).
 */
export function announceLibraryChange(): void {
  window.dispatchEvent(new Event(LIBRARY_CHANGED));
}

export function useLibraryChange(listener: () => void): void {
  const latest = useRef(listener);
  latest.current = listener;
  useEffect(() => {
    const run = () => latest.current();
    window.addEventListener(LIBRARY_CHANGED, run);
    return () => window.removeEventListener(LIBRARY_CHANGED, run);
  }, []);
}
