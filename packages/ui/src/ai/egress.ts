// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Egress-receipt vocabulary shared by the desktop Privacy group and the
 * mobile Privacy screens. The log both hosts render is append-only:
 * viewers are read-only and never offer deletion or an off switch, and the
 * table they read from structurally cannot hold prompt or response content.
 */
import type { AiEgressEntry } from '@typvia/shared';
import type { Locale, Tr } from '../i18n';

/** Wire classes → row wording; unknown classes print as-is (forward
 *  compatibility, never hidden). */
export function egressClassLabel(requestClass: string, tr: Tr): string {
  if (requestClass === 'completion') return tr('AI request', 'AI 请求');
  if (requestClass === 'connectivity') return tr('Connectivity check', '联网检查');
  return requestClass;
}

export function egressEntryTime(occurredAt: number, locale: Locale): string {
  return new Date(occurredAt).toLocaleString(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}

/** Entries newer than local midnight, from the newest-first first page. */
export function egressTodayCount(entries: AiEgressEntry[]): number {
  const midnight = new Date();
  midnight.setHours(0, 0, 0, 0);
  return entries.filter((entry) => entry.occurredAt >= midnight.getTime()).length;
}
