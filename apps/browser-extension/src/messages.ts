// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Wire types shared by the popup and the service worker: the native host's
 * message surface (snake_case as serialized by the host) and the
 * popup↔worker envelope. No logic here —
 * the host owns search/render semantics.
 */

/** Registered native-messaging host name (manifest `name` field). */
export const HOST_NAME = 'dev.typvia.browser_host';

export type HostRequest =
  | { type: 'hello' }
  | { type: 'search'; query: string; limit?: number }
  | { type: 'list_recent'; limit?: number }
  | { type: 'list_favorites'; limit?: number }
  | { type: 'render'; snippet_id: string; variables?: Record<string, string> };

export interface HostResultEntry {
  id: string;
  title: string;
  snippet_type: string;
  trigger: string | null;
  is_favorite: boolean;
  preview: string;
  /** Template variable names to collect before rendering. */
  variables: string[];
}

export interface HostError {
  ok: false;
  code: string;
  field?: string;
}

export type HostResponse =
  | {
      ok: true;
      protocol_version: number;
      snapshot_present: boolean;
      generated_at: number | null;
      snippet_count: number | null;
    }
  | { ok: true; results: HostResultEntry[] }
  | { ok: true; text: string }
  | HostError;

/** Popup → service-worker envelope. */
export type WorkerMessage =
  { kind: 'host'; request: HostRequest } | { kind: 'insert'; text: string };

/** Reply for `insert`: whether the page accepted a DOM-level insertion. */
export interface InsertReply {
  inserted: boolean;
}
