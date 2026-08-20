/**
 * WebView side of the mobile sync schedule. The host runs one
 * round shortly after launch and one on every `Resumed` lifecycle event; this
 * layer adds the WebView's own view of coming back to the foreground
 * (`visibilitychange` → visible), which also covers a page reload and the
 * cases where the activity was never suspended.
 *
 * There is no polling loop and no background task on mobile: iOS
 * BGTaskScheduler and Android WorkManager are out of scope here, so
 * foreground returns plus the manual "Sync now" are the whole schedule.
 *
 * Failures are swallowed: a round that cannot reach the server is the offline
 * state, not an event worth interrupting anyone for, and the Sync screen
 * reports the backlog either way.
 */
import { syncNow } from '@typvia/shared';

/** Ignore a foreground return that lands within this window of the last
 * round, so tab switching does not queue rounds back to back. */
export const SYNC_TRIGGER_COOLDOWN_MS = 30_000;

export interface SyncTrigger {
  /** Starts listening for foreground returns. */
  start: () => void;
  /** Stops listening and ignores any in-flight round (unmount). */
  stop: () => void;
}

export function createSyncTrigger(
  run: () => Promise<unknown> = syncNow,
  clock: () => number = Date.now,
): SyncTrigger {
  let stopped = false;
  let inFlight = false;
  let lastRunAt: number | null = null;

  const round = () => {
    if (stopped || inFlight) return;
    const at = clock();
    if (lastRunAt !== null && at - lastRunAt < SYNC_TRIGGER_COOLDOWN_MS) return;
    lastRunAt = at;
    inFlight = true;
    run()
      .catch(() => undefined)
      .finally(() => {
        inFlight = false;
      });
  };

  const onVisibility = () => {
    if (document.visibilityState === 'visible') round();
  };

  return {
    start() {
      document.addEventListener('visibilitychange', onVisibility);
    },
    stop() {
      stopped = true;
      document.removeEventListener('visibilitychange', onVisibility);
    },
  };
}
