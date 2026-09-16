// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

import type { SearchHit, Snippet, SyncStatus } from '@typvia/shared';
import { trFor, type Locale, type Tr } from '@typvia/ui';
import type { LibraryScope } from './use-snippet-pages';
import { whenLabel } from './when-label';

/** A collection as the choice group names it: everything, the unsorted, or one folder. */
export type CollectionKey = 'all' | 'unsorted' | `folder:${string}`;

export function scopeFor(key: CollectionKey): LibraryScope {
  if (key === 'all') return { view: 'all', folderId: null };
  if (key === 'unsorted') return { view: 'unsorted', folderId: null };
  return { view: 'folder', folderId: key.slice('folder:'.length) };
}

/** Code keeps its own face: indentation and spacing have to read exactly. */
export function isCode(snippet: Pick<Snippet, 'snippetType'>): boolean {
  return snippet.snippetType === 'code' || snippet.snippetType === 'command';
}

/** The line a row shows: what the reader owns is the body, not the title. */
export function firstLine(snippet: Snippet): string {
  const line = (snippet.body ?? '').split('\n').find((text) => text.trim() !== '');
  return line ?? snippet.title;
}

export function collectionNameOf(
  snippet: Snippet,
  folderNames: ReadonlyMap<string, string>,
  tr: Tr,
): string {
  return (
    (snippet.folderId !== null && folderNames.get(snippet.folderId)) || tr('Unsorted', '待整理')
  );
}

export function usedLabel(snippet: Snippet, now: number, locale: Locale): string {
  const tr = trFor(locale);
  if (snippet.lastUsedAt === null || snippet.usageCount === 0)
    return tr('Not used yet', '还没用过');
  const when = whenLabel(snippet.lastUsedAt, now, locale);
  const count = snippet.usageCount;
  return tr(
    count === 1 ? `Used once · ${when}` : `Used ${String(count)} times · ${when}`,
    `用过 ${String(count)} 次 · ${when}`,
  );
}

export interface SearchGroups {
  body: Snippet[];
  trigger: Snippet[];
}

const byRecency = (a: Snippet, b: Snippet) => (b.lastUsedAt ?? -1) - (a.lastUsedAt ?? -1);

/**
 * Splits one result set into "in the text" and "in the trigger" using the tier
 * the core already reported for each hit, then shows each group most recently
 * used first — a reader recognises what they used last, not a match score.
 */
export function groupResults(rows: readonly Snippet[], hits: readonly SearchHit[]): SearchGroups {
  const byTrigger = new Set(
    hits.filter((hit) => hit.tier === 'trigger').map((hit) => hit.snippetId),
  );
  return {
    body: rows.filter((row) => !byTrigger.has(row.id)).sort(byRecency),
    trigger: rows.filter((row) => byTrigger.has(row.id)).sort(byRecency),
  };
}

/** `paused` and `warn` are the two states where sync is asleep rather than working. */
export type SyncTone = 'ok' | 'warn' | 'idle' | 'paused';

/** The sync state in one line, with its dot — the colour is never the only signal. */
export function syncLine(
  status: SyncStatus | null,
  now: number,
  locale: Locale,
): { text: string; tone: SyncTone } | null {
  const tr = trFor(locale);
  if (status === null) return null;
  if (!status.configured) return { text: tr('Only on this Mac', '只在这台 Mac 上'), tone: 'idle' };
  if (!status.enabled) return { text: tr('Sync is off', '同步关着'), tone: 'paused' };
  if (status.pendingBacklog > 0) {
    const n = String(status.pendingBacklog);
    return { text: tr(`${n} not sent yet`, `还有 ${n} 条没送上去`), tone: 'warn' };
  }
  if (status.lastSyncAt === null) return { text: tr('Synced', '已同步'), tone: 'ok' };
  const when = whenLabel(status.lastSyncAt, now, locale);
  return { text: tr(`Synced · ${when}`, `已同步 · ${when}`), tone: 'ok' };
}

/** The trigger column never gets narrower than this, so short triggers keep the rows aligned. */
export const TRIGGER_MIN_CHARS = 4;
/** Past this many characters a trigger ellipsizes rather than squeeze the text. */
export const TRIGGER_MAX_CHARS = 14;

/** Characters the trigger column needs to show the longest of these triggers whole. */
export function triggerChars(triggers: readonly (string | null)[]): number {
  const longest = triggers.reduce(
    (widest, trigger) => Math.max(widest, [...(trigger ?? '')].length),
    0,
  );
  return Math.min(TRIGGER_MAX_CHARS, Math.max(TRIGGER_MIN_CHARS, longest));
}
