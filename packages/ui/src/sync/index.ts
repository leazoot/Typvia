// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Shared sync UI layer. Desktop and mobile draw sync differently —
 * a route with a rail vs. a settings list on a phone — but the flows, the
 * derived labels and every sentence are identical, so they exist once here
 * and each host only contributes its layout.
 *
 * This module is the one part of `@typvia/ui` that reaches the typed IPC
 * client; it is exported under `@typvia/ui/sync` so the purely presentational
 * components stay free of it.
 */
export { SasCompare } from './sas-compare';
export {
  offlineNotice,
  conflictHeadline,
  deviceRowMeta,
  headline,
  isOfflineError,
  messageOf,
  revokeConfirmation,
  revokeNotice,
  rotateNotice,
  roundNotice,
  selfDevice,
  selfStatus,
  shapeLabel,
} from './labels';
export { ageLabel, relativeTime } from './relative-time';
export { useConflictDecision } from './use-conflict-decision';
export type { ConflictDecision } from './use-conflict-decision';
export { usePairAdmit, usePairJoin } from './use-pairing';
export type { PairAdmit, PairJoin } from './use-pairing';
export { useAccountRecover, useRecoveryExport } from './use-recovery';
export type { AccountRecover, RecoveryExport } from './use-recovery';
export { useSyncOverview } from './use-sync-overview';
export type { SyncOverview } from './use-sync-overview';
export { backlogLabel, conflictCountLabel, syncBadge, useSyncSettings } from './use-sync-settings';
export type { SyncSettings } from './use-sync-settings';
export { useSyncSetup, webdavCredentials } from './use-sync-setup';
export type { SyncBackendKind, SyncSetupFlow } from './use-sync-setup';
