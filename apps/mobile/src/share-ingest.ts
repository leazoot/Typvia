// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * WebView side of the share-extension inbox drain: asks the host
 * to ingest pending shared text once when the shell is ready and again every
 * time the app returns to the foreground (visibilitychange → visible). Only
 * counts cross this layer — the shared content itself never reaches the
 * frontend outside normal list fetches.
 */
import { shareInboxIngest } from '@typvia/shared';

export interface ShareIngestor {
  /** Runs one immediate drain and starts listening for foreground returns. */
  start: () => void;
  /** Stops listening and ignores any in-flight result (unmount). */
  stop: () => void;
}

export function createShareIngestor(
  onIngested: (count: number) => void,
  ingest: () => Promise<number> = shareInboxIngest,
): ShareIngestor {
  let stopped = false;
  let inFlight = false;
  const drain = () => {
    if (stopped || inFlight) return;
    inFlight = true;
    ingest()
      .then((count) => {
        if (!stopped && count > 0) onIngested(count);
      })
      // Best-effort by design: the inbox files stay put host-side and the
      // next foreground drain retries; nothing here can lose a share.
      .catch(() => undefined)
      .finally(() => {
        inFlight = false;
      });
  };
  const onVisibility = () => {
    if (document.visibilityState === 'visible') drain();
  };
  return {
    start() {
      document.addEventListener('visibilitychange', onVisibility);
      drain();
    },
    stop() {
      stopped = true;
      document.removeEventListener('visibilitychange', onVisibility);
    },
  };
}
