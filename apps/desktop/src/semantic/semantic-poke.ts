// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

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
