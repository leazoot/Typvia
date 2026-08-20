/**
 * App-wide debounced embedding-queue poke: mutations coalesce
 * into one background drain, same burst semantics as the other derived
 * surfaces. The host command no-ops without a model, so callers never
 * gate on state.
 */
import { createSnapshotRefresher, semanticSync } from '@typvia/shared';

const refresher = createSnapshotRefresher(semanticSync);

/** Called after any snippet mutation; coalesces into one queue drain. */
export function notifySemanticMutation(): void {
  refresher.notifyMutation();
}
