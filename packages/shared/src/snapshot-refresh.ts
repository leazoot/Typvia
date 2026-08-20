/**
 * Debounced snapshot refresh after data mutations, mirroring the desktop
 * espanso notifyMutation semantics (900ms trailing): bursts of edits
 * coalesce into one host write. The refresh function is injected so each
 * host wires its own snapshot command (`snapshotRefresh` on mobile,
 * `browserIntegrationSync` on desktop) and tests mock at the IPC layer.
 */
/** Coalesce bursts of edits into one snapshot write (desktop parity). */
export const SNAPSHOT_REFRESH_DEBOUNCE_MS = 900;

export interface SnapshotRefresher {
  /** Called after a snippet mutation; (re)schedules a trailing refresh. */
  notifyMutation: () => void;
  /** Drops any pending refresh (component unmount). */
  cancel: () => void;
}

export function createSnapshotRefresher(refresh: () => Promise<void>): SnapshotRefresher {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return {
    notifyMutation() {
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        // Best-effort by design (desktop notifyMutation parity): a failed
        // write keeps the previous snapshot in place and the save that
        // triggered this already succeeded, so nothing blocks the UX.
        void refresh().catch(() => undefined);
      }, SNAPSHOT_REFRESH_DEBOUNCE_MS);
    },
    cancel() {
      if (timer !== null) clearTimeout(timer);
      timer = null;
    },
  };
}
