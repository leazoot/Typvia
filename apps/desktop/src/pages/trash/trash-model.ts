// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { Snippet } from '@typvia/shared';

/** Deleted snippets are cleared this many days after they were deleted. */
export const RETENTION_DAYS = 30;
/** Fewer days left than this and the row carries the amber dot. */
export const SOON_DAYS = 7;
const DAY_MS = 24 * 60 * 60 * 1000;

/** Whole days before a deleted snippet is cleared, never below zero. */
export function daysLeft(deletedAt: number, now: number): number {
  return Math.max(0, Math.ceil((deletedAt + RETENTION_DAYS * DAY_MS - now) / DAY_MS));
}

/** How many are in the trash and how long the oldest of them has left. */
export function trashSummary(
  rows: readonly Snippet[],
  now: number,
): { count: number; soonest: number | null } {
  let soonest: number | null = null;
  for (const row of rows) {
    if (row.deletedAt === null) continue;
    const left = daysLeft(row.deletedAt, now);
    if (soonest === null || left < soonest) soonest = left;
  }
  return { count: rows.length, soonest };
}
