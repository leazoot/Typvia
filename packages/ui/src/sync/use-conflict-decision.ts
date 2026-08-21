// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Conflict arbitration as a headless machine. The layout
 * differs — two columns on desktop, stacked on a phone — but the decision,
 * the refresh after it and the promise that nothing is deleted are the same.
 */
import {
  type ConflictKeep,
  type ConflictPair,
  syncConflictResolve,
  syncConflicts,
} from '@typvia/shared';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { trFor, useLocale } from '../i18n';

export interface ConflictDecision {
  /** `null` until the first answer — the loading state. */
  pairs: ConflictPair[] | null;
  busy: boolean;
  error: string | null;
  /** Timestamp the relative labels are rendered against; stable per mount. */
  now: number;
  resolve: (pair: ConflictPair, keep: ConflictKeep) => Promise<void>;
}

export function useConflictDecision(): ConflictDecision {
  const locale = useLocale();
  // Memoized so the callbacks below can depend on it without re-creating
  // themselves every render.
  const tr = useMemo(() => trFor(locale), [locale]);
  const [pairs, setPairs] = useState<ConflictPair[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const now = useRef(Date.now()).current;

  const refresh = useCallback(async () => {
    setPairs(await syncConflicts());
  }, []);

  useEffect(() => {
    void refresh().catch(() => {
      setPairs([]);
      setError(
        tr(
          'The conflict list could not be read. Both versions are still on disk.',
          '冲突列表暂时读不出来。两个版本都仍完整保存在本机。',
        ),
      );
    });
  }, [refresh, tr]);

  const resolve = useCallback(
    async (pair: ConflictPair, keep: ConflictKeep) => {
      setBusy(true);
      setError(null);
      try {
        await syncConflictResolve(pair.copy.id, keep);
        await refresh();
      } catch (caught) {
        setError(
          caught instanceof Error
            ? tr(
                `${caught.message} Both versions are unchanged.`,
                `${caught.message} 两个版本都没有改动。`,
              )
            : tr('Nothing was changed.', '没有任何改动。'),
        );
      } finally {
        setBusy(false);
      }
    },
    [refresh, tr],
  );

  return { pairs, busy, error, now, resolve };
}
