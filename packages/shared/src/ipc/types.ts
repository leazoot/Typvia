// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * IPC data shapes, mirroring the Rust DTOs in apps/desktop/src-tauri/src/dto.rs
 * (serde camelCase). A sensitive snippet's `body` is always `null` on this side
 * of the boundary; its plaintext is only ever obtained through `vaultReveal` on
 * an unlocked session.
 */

export interface Snippet {
  id: string;
  title: string;
  body: string | null;
  snippetType: string;
  securityLevel: string;
  description: string | null;
  folderId: string | null;
  trigger: string | null;
  triggerMode: string | null;
  language: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  isEnabled: boolean;
  createdAt: number;
  updatedAt: number;
  lastUsedAt: number | null;
  usageCount: number;
  version: number;
  deletedAt: number | null;
}

export interface SnippetCreateInput {
  title: string;
  body: string;
  snippetType: string;
  description?: string | null;
  folderId?: string | null;
  trigger?: string | null;
  triggerMode?: string | null;
  language?: string | null;
}

export interface SnippetUpdateInput {
  id: string;
  title: string;
  body: string;
  snippetType: string;
  description?: string | null;
  folderId?: string | null;
  trigger?: string | null;
  triggerMode?: string | null;
  language?: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  isEnabled: boolean;
}

export interface Folder {
  id: string;
  parentId: string | null;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface FolderCreateInput {
  name: string;
  parentId?: string | null;
  sortOrder: number;
}

export interface FolderUpdateInput {
  id: string;
  name: string;
  parentId?: string | null;
  sortOrder: number;
}

export interface Tag {
  id: string;
  name: string;
  createdAt: number;
}

/** The Library rail vocabulary: four saved views plus single-folder scope. */
export type LibraryView = 'all' | 'recent' | 'starred' | 'unsorted' | 'folder';

export interface FolderCount {
  folderId: string;
  count: number;
}

export interface LibraryCounts {
  total: number;
  recent: number;
  starred: number;
  unsorted: number;
  trash: number;
  folders: FolderCount[];
}

export type SearchTier = 'title_exact' | 'trigger' | 'title_prefix' | 'tag' | 'content';

export interface SearchHit {
  snippetId: string;
  title: string;
  tier: SearchTier;
  isSensitive: boolean;
}

/**
 * Observable vault-session state (mirrors VaultStatusDto). Carries no key
 * material — only whether a device vault exists, whether it is unlocked, and
 * the timestamps the Vault page uses to render the auto-lock countdown.
 */
export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
  unlockedAt: number | null;
  lastActivityAt: number | null;
  idleTimeoutMs: number;
}

/**
 * Mobile host boot smoke: proves the on-device database migrated
 * and the snippet store answers. Served only by the mobile shell.
 */
export interface MobileBootstrap {
  schemaVersion: number;
  snippetTotal: number;
}
