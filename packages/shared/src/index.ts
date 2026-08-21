// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

// Shared TypeScript types and the typed IPC layer. Package entry point.
export * from './ipc';
export { APP_ROUTES } from './routes';
export { SNAPSHOT_REFRESH_DEBOUNCE_MS, createSnapshotRefresher } from './snapshot-refresh';
export type { SnapshotRefresher } from './snapshot-refresh';
export type { AppRoute } from './routes';
