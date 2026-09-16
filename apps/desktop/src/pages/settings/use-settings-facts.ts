// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import { libraryCounts } from '@typvia/shared';
import { useEffect, useState } from 'react';

/** Snippets in the library, or null until read. */
export function useLibraryTotal(): number | null {
  const [total, setTotal] = useState<number | null>(null);

  useEffect(() => {
    let live = true;
    libraryCounts()
      .then((counts) => {
        if (live) setTotal(counts.total);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  return total;
}
